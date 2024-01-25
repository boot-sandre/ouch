use std::{
    io::{self, BufReader, Read},
    ops::ControlFlow,
    path::{Path, PathBuf},
};

use fs_err as fs;

use crate::{
    commands::{warn_user_about_loading_sevenz_in_memory, warn_user_about_loading_zip_in_memory},
    extension::{
        split_first_compression_format,
        CompressionFormat::{self, *},
        Extension,
    },
    info,
    utils::{self, nice_directory_display, user_wants_to_continue},
    QuestionAction, QuestionPolicy, BUFFER_CAPACITY,
};

// Decompress a file
//
// File at input_file_path is opened for reading, example: "archive.tar.gz"
// formats contains each format necessary for decompression, example: [Gz, Tar] (in decompression order)
// output_dir it's where the file will be decompressed to, this function assumes that the directory exists
// output_file_path is only used when extracting single file formats, not archive formats like .tar or .zip
pub fn decompress_file(
    input_file_path: &Path,
    formats: Vec<Extension>,
    output_dir: &Path,
    output_file_path: PathBuf,
    question_policy: QuestionPolicy,
    quiet: bool,
) -> crate::Result<()> {
    assert!(output_dir.exists());
    let reader = fs::File::open(input_file_path)?;

    // Zip archives are special, because they require io::Seek, so it requires it's logic separated
    // from decoder chaining.
    //
    // This is the only case where we can read and unpack it directly, without having to do
    // in-memory decompression/copying first.
    //
    // Any other Zip decompression done can take up the whole RAM and freeze ouch.
    if let [Extension {
        compression_formats: [Zip],
        ..
    }] = formats.as_slice()
    {
        let zip_archive = zip::ZipArchive::new(reader)?;
        let files_unpacked = if let ControlFlow::Continue(files) = smart_unpack(
            |output_dir| crate::archive::zip::unpack_archive(zip_archive, output_dir, quiet),
            output_dir,
            &output_file_path,
            question_policy,
        )? {
            files
        } else {
            return Ok(());
        };

        // this is only printed once, so it doesn't result in much text. On the other hand,
        // having a final status message is important especially in an accessibility context
        // as screen readers may not read a commands exit code, making it hard to reason
        // about whether the command succeeded without such a message
        info!(
            accessible,
            "Successfully decompressed archive in {} ({} files).",
            nice_directory_display(output_dir),
            files_unpacked
        );

        return Ok(());
    }

    // Will be used in decoder chaining
    let reader = BufReader::with_capacity(BUFFER_CAPACITY, reader);
    let mut reader: Box<dyn Read> = Box::new(reader);

    // Grab previous decoder and wrap it inside of a new one
    let chain_reader_decoder = |format: &CompressionFormat, decoder: Box<dyn Read>| -> crate::Result<Box<dyn Read>> {
        let decoder: Box<dyn Read> = match format {
            Gzip => Box::new(flate2::read::GzDecoder::new(decoder)),
            Bzip => Box::new(bzip2::read::BzDecoder::new(decoder)),
            Lz4 => Box::new(lz4_flex::frame::FrameDecoder::new(decoder)),
            Lzma => Box::new(xz2::read::XzDecoder::new(decoder)),
            Snappy => Box::new(snap::read::FrameDecoder::new(decoder)),
            Zstd => Box::new(zstd::stream::Decoder::new(decoder)?),
            Tar | Zip | Rar | SevenZip => unreachable!(),
        };
        Ok(decoder)
    };

    let (first_extension, extensions) = split_first_compression_format(&formats);

    for format in extensions.iter().rev() {
        reader = chain_reader_decoder(format, reader)?;
    }

    let files_unpacked = match first_extension {
        Gzip | Bzip | Lz4 | Lzma | Snappy | Zstd => {
            reader = chain_reader_decoder(&first_extension, reader)?;

            let mut writer = match utils::ask_to_create_file(&output_file_path, question_policy)? {
                Some(file) => file,
                None => return Ok(()),
            };

            io::copy(&mut reader, &mut writer)?;

            1
        }
        Tar => {
            if let ControlFlow::Continue(files) = smart_unpack(
                |output_dir| crate::archive::tar::unpack_archive(reader, output_dir, quiet),
                output_dir,
                &output_file_path,
                question_policy,
            )? {
                files
            } else {
                return Ok(());
            }
        }
        Zip => {
            if formats.len() > 1 {
                warn_user_about_loading_zip_in_memory();

                if !user_wants_to_continue(input_file_path, question_policy, QuestionAction::Decompression)? {
                    return Ok(());
                }
            }

            let mut vec = vec![];
            io::copy(&mut reader, &mut vec)?;
            let zip_archive = zip::ZipArchive::new(io::Cursor::new(vec))?;

            if let ControlFlow::Continue(files) = smart_unpack(
                |output_dir| crate::archive::zip::unpack_archive(zip_archive, output_dir, quiet),
                output_dir,
                &output_file_path,
                question_policy,
            )? {
                files
            } else {
                return Ok(());
            }
        }
        #[cfg(feature = "unrar")]
        Rar => {
            type UnpackResult = crate::Result<usize>;
            let unpack_fn: Box<dyn FnOnce(&Path) -> UnpackResult> = if formats.len() > 1 {
                let mut temp_file = tempfile::NamedTempFile::new()?;
                io::copy(&mut reader, &mut temp_file)?;
                Box::new(move |output_dir| crate::archive::rar::unpack_archive(temp_file.path(), output_dir, quiet))
            } else {
                Box::new(|output_dir| crate::archive::rar::unpack_archive(input_file_path, output_dir, quiet))
            };

            if let ControlFlow::Continue(files) =
                smart_unpack(unpack_fn, output_dir, &output_file_path, question_policy)?
            {
                files
            } else {
                return Ok(());
            }
        }
        #[cfg(not(feature = "unrar"))]
        Rar => {
            return Err(crate::archive::rar_stub::no_support());
        }
        SevenZip => {
            if formats.len() > 1 {
                warn_user_about_loading_sevenz_in_memory();

                if !user_wants_to_continue(input_file_path, question_policy, QuestionAction::Decompression)? {
                    return Ok(());
                }
            }

            let mut vec = vec![];
            io::copy(&mut reader, &mut vec)?;

            if let ControlFlow::Continue(files) = smart_unpack(
                |output_dir| crate::archive::sevenz::decompress_sevenz(io::Cursor::new(vec), output_dir, quiet),
                output_dir,
                &output_file_path,
                question_policy,
            )? {
                files
            } else {
                return Ok(());
            }
        }
    };

    // this is only printed once, so it doesn't result in much text. On the other hand,
    // having a final status message is important especially in an accessibility context
    // as screen readers may not read a commands exit code, making it hard to reason
    // about whether the command succeeded without such a message
    info!(
        accessible,
        "Successfully decompressed archive in {}.",
        nice_directory_display(output_dir)
    );
    info!(accessible, "Files unpacked: {}", files_unpacked);

    Ok(())
}

/// Unpacks an archive with some heuristics
/// - If the archive contains only one file, it will be extracted to the `output_dir`
/// - If the archive contains multiple files, it will be extracted to a subdirectory of the
///   output_dir named after the archive (given by `output_file_path`)
/// Note: This functions assumes that `output_dir` exists
fn smart_unpack(
    unpack_fn: impl FnOnce(&Path) -> crate::Result<usize>,
    output_dir: &Path,
    output_file_path: &Path,
    question_policy: QuestionPolicy,
) -> crate::Result<ControlFlow<(), usize>> {
    assert!(output_dir.exists());
    match fs::create_dir(&output_dir) {
        Ok(_) => info!(accessible, "Directory created at {}", output_dir.display()),
        Err(e) => info!(accessible, "Failed to create directory at {}, error: {}", output_dir.display(), e),
    }
    let files = unpack_fn(output_dir)?;

    Ok(ControlFlow::Continue(files))
}
