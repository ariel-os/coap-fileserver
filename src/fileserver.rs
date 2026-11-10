use coap_message::{Code as _, OptionNumber as _};
use coap_message::{
    MessageOption as _, MinimalWritableMessage, MutableWritableMessage, ReadableMessage,
};
use coap_message_utils::{
    Error, OptionsExt as _,
    option_value::{Block2RequestData, Block2ResponseData},
};

use cap_std::fs_utf8::{File, Metadata, Dir};

use std::io::{Read as _, Seek as _, SeekFrom};

fn errmap(e: std::io::Error) -> Error {
    let converted = match e.kind() {
        std::io::ErrorKind::NotFound => Error::not_found(),
        std::io::ErrorKind::PermissionDenied => Error::unauthorized(),
        std::io::ErrorKind::NotADirectory => Error::bad_request().with_title("not a directory"),
        std::io::ErrorKind::IsADirectory => Error::bad_request().with_title("is a directory"),
        // std::io::ErrorKind::NotSeekable => … might need actra handling
        _ => Error::internal_server_error(), // or bad request, hard to find a good splitting point here
    };
    log::error!("Serving IO error {:?} as {:?}", e, converted);
    converted
}

fn file_etag(_file: &File, metadata: &Metadata) -> Result<ETag, Error> {
    if metadata.file_type().is_dir() {
        return Err(Error::bad_request().with_title("is a directory"));
    }

    // FIXME:
    // * Add inode where usable
    // * Allow configuration to do full hashing instead (more effort server-side)
    // * Allow using fs-verity (or another sealing mechanism?) for guaranteed *and* efficient ETags.
    //
    // Also, there is a bit of a race here because content is read only after reading the metadata,
    // but it'd be racy either way; fs-verity would neatly catch that too.
    let hash_this = (metadata.modified().map_err(errmap)?, metadata.len());

    const CRC: crc::Crc<u64> = crc::Crc::<u64>::new(&crc::CRC_64_XZ);
    let mut etag = CRC.digest();
    feed_digest_with_hash(&mut etag, hash_this);
    Ok(etag.finalize().to_le_bytes())
}

/// Glue code between CRC's digest trait and the [`std::hash::Hash`] trait
fn feed_digest_with_hash(etag: &mut crc::Digest<'_, u64>, hash_this: impl std::hash::Hash) {
    struct DigestHasher<'a, 'b>(&'a mut crc::Digest<'b, u64>);

    impl<'a, 'b> std::hash::Hasher for DigestHasher<'a, 'b> {
        fn finish(&self) -> u64 {
            unreachable!("not used");
        }

        fn write(&mut self, bytes: &[u8]) {
            self.0.update(bytes);
        }
    }

    let mut hasher = DigestHasher(etag);

    hash_this.hash(&mut hasher);
}

pub struct FileServer {
    base: Dir,
}

impl FileServer {
    pub fn new(base: Dir) -> Self {
        Self { base }
    }
}

type ETag = [u8; 8];

#[derive(Debug)]
enum FileOrDir {
    File(File),
    Dir(Dir),
}

pub enum Outcome {
    RenderFile {
        /// The file to serve, already pre-seeked.
        file: File,
        /// The pre-calculated length we will send. If that can not be read, an internal server
        /// error is to be raised. (We could also read on demand, but we committed to a size2 already
        /// anyway, and to an etag that incoprorates this length, so if a truncation happens right
        /// in the middle, yeah, 5.00).
        length: u16,
        size2: u64,
        block2: Option<Block2ResponseData>,
        etag: ETag,
    },
    RenderValid {
        etag: ETag,
    },
    RenderIndex {
        // Not calculating an ETag here: we're letting the writer-with-etag handle this. (That
        // implicitly makes sure we're covering the right data in it).
        dir: Dir,
        block2: Block2RequestData,
    },
}

impl coap_handler::Handler for FileServer {
    type RequestData = Outcome;
    type ExtractRequestError = Error;
    type BuildResponseError<M: MinimalWritableMessage> = M::UnionError;

    fn extract_request_data<M: ReadableMessage>(
        &mut self,
        request: &M,
    ) -> Result<Self::RequestData, Self::ExtractRequestError> {
        if request.code().into() != coap_numbers::code::GET {
            return Err(Error::method_not_allowed());
        }

        let mut path = self
            .base
            .try_clone()
            // If this fails, the directory being served likely just went away.
            // (Not `?`ing it because we need this as a result anyway).
            .map_err(|_| Error::internal_server_error());
        // While ingesting Uri-Path options we don't know yet whether the pending segment wil be
        // traversed as a directory or is a file.
        let mut next_segment = None;

        // BTreeMap would be more efficient in lookup, but we expect 0 or 1 or at most a few.
        let mut requested_etags = Vec::<ETag>::new();

        let mut block2 = None;

        request
            .options()
            .take_block2(&mut block2)
            .take_uri_path(|seg| {
                let Ok(base_path) = path.as_ref() else {
                    // Once we hit an error, we leave that error in and ignore the rest of the
                    // path.
                    return;
                };

                // FIXME: Find something better than cap_std that has API that only walks single
                // items at a time. (After all, how can we be sure that / and \ are exhaustive
                // platform path separators?).
                if seg.contains('/') || seg.contains('\\') {
                    // We generally rely on cap_std to restrict us according to the server's
                    // settings; however, in corner cases (e.g. when an access policy deviates from
                    // best practices and uses a denylist for paths), variability from accepting
                    // encoded slashes would be detrimental.
                    log::error!(
                        "Segment contains slashes. While this is allowed in CoAP, even cap_std "
                    );
                    path = Err(Error::not_found());
                    return;
                }

                if let Some(prev_segment) = next_segment.take() {
                    log::debug!("Traversing into directory {:?}", prev_segment);
                    path = base_path.open_dir(prev_segment).map_err(errmap)
                }

                next_segment = Some(seg.to_owned());
            })
            .filter(|o| {
                if o.number() == coap_numbers::option::ETAG {
                    // Only accepting the [u8; 8] shaped ETags that we use, ignoring any other
                    if let Ok(value) = o.value().try_into() {
                        requested_etags.push(value);
                    }
                    false
                } else {
                    true
                }
            })
            .ignore_elective_others()?;

        let path = path?;
        let block2 = block2.unwrap_or_default();

        let entity = match next_segment {
            Some(filename) if filename != "" => {
                log::debug!("Opening file {:?}", filename);
                FileOrDir::File(path.open(filename).map_err(errmap)?)
            }
            _ => FileOrDir::Dir(path),
        };

        match entity {
            FileOrDir::File(mut file) => {
                let metadata = file.metadata().map_err(errmap)?;

                let etag = file_etag(&file, &metadata)?;

                if requested_etags.contains(&etag) {
                    Ok(Outcome::RenderValid { etag: etag })
                } else {
                    let start: u64 = block2
                        .start()
                        .try_into()
                        // This is practically unreachable: Conversion is fallible only on 128
                        // bit systems, and starts >64bit can not be expressed in CoAP anyway
                        // due to option length limits for block options.
                        .map_err(|_| Error::bad_request())?;
                    file.seek(SeekFrom::Start(start)).map_err(errmap)?;

                    let size2 = metadata.len();
                    let available = size2
                        .checked_sub(start)
                        // This would only fail if the seek above succeeded spuriously, eg. because
                        // the file was just appended to.
                        .ok_or(Error::internal_server_error())?;

                    let available = u16::try_from(available).unwrap_or(u16::MAX);
                    // Clamping the lower end to 1 conveniently causes an error later when
                    // attempting to read -- this is CoAP and not USB, and the end of the file is
                    // signalled with the last block, not an empty read beyond it.
                    let length = available.clamp(1, block2.size());

                    let block2 = block2.to_response(available > length);
                    let block2 = if block2.start() == 0 && block2.more() == false {
                        // No point in sending that for small files
                        None
                    } else {
                        Some(block2)
                    };

                    log::info!("Serving file {:?} (from {} to {})", file, start, length);

                    Ok(Outcome::RenderFile {
                        file: file,
                        etag: etag,
                        size2,
                        length,
                        block2,
                    })
                }
            }
            FileOrDir::Dir(dir) => Ok(Outcome::RenderIndex { dir, block2 }),
        }
    }

    fn estimate_length(&mut self, _request: &Self::RequestData) -> usize {
        // Not really relevant for the server we run on
        1200
    }

    fn build_response<M: MutableWritableMessage>(
        &mut self,
        response: &mut M,
        request: Self::RequestData,
    ) -> Result<(), Self::BuildResponseError<M>> {
        match request {
            Outcome::RenderValid { etag } => {
                response.set_code(M::Code::new(coap_numbers::code::VALID)?);
                response.add_option(M::OptionNumber::new(coap_numbers::option::ETAG)?, &etag)?;
                Ok(())
            }

            Outcome::RenderFile {
                mut file,
                length,
                size2,
                block2,
                etag,
            } => {
                response.set_code(M::Code::new(coap_numbers::code::CONTENT)?);
                response.add_option(M::OptionNumber::new(coap_numbers::option::ETAG)?, &etag)?;
                if let Some(block2) = block2 {
                    response.add_option_uint(
                        M::OptionNumber::new(coap_numbers::option::BLOCK2)?,
                        block2.to_option_value(),
                    )?;
                    if block2.start() == 0 {
                        response.add_option_uint(
                            M::OptionNumber::new(coap_numbers::option::SIZE2)?,
                            size2,
                        )?;
                    }
                }
                let length = length.into();
                let payload = response.payload_mut_with_len(length)?;
                if file.read(payload).is_ok_and(|l| l == length) {
                    Ok(())
                } else {
                    // FIXME: Rewind options? (Should be easier with next Message version)
                    response.set_code(M::Code::new(coap_numbers::code::INTERNAL_SERVER_ERROR)?);
                    response.truncate(0)?;
                    Ok(())
                }
            }
            Outcome::RenderIndex { dir, block2 } => {
                response.set_code(M::Code::new(coap_numbers::code::CONTENT)?);

                let result = coap_handler_implementations::helpers::block2_write(
                    block2,
                    response,
                    |w| -> Result<(), std::io::Error> {
                        use core::fmt::Write;

                        writeln!(w, "Index:").unwrap();
                        for e in dir.entries()? {
                            writeln!(w, "{}", e?.file_name()?).unwrap();
                        }

                        Ok(())
                    },
                );

                if result.is_err() {
                    // FIXME: Rewind options? (Should be easier with next Message version)
                    response.set_code(M::Code::new(coap_numbers::code::INTERNAL_SERVER_ERROR)?);
                    response.truncate(0)?;
                }

                Ok(())
            }
        }
    }
}
