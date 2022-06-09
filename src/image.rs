use anyhow::anyhow;
use ffmpeg_next::media::Type;
use ffmpeg_next::Error;
use ffmpeg_next::{
    codec,
    format::{input, Pixel},
    frame::Video,
    software::scaling::Flags,
};
use ffmpeg_sys_next::avcodec_parameters_to_context;
use std::path::Path;
pub fn read_image(path: &Path) -> anyhow::Result<Video> {
    let mut ictx = input(&path).map_err(|e| anyhow!("Failed to open file: {}", e))?;
    let input = ictx
        .streams()
        .best(Type::Video)
        .ok_or(anyhow!("Failed to find video stream"))?;
    let video_stream_index = input.index();
    let context_decoder = {
        let parameters = input.parameters();
        let mut context = codec::context::Context::new();

        unsafe {
            match avcodec_parameters_to_context(context.as_mut_ptr(), parameters.as_ptr()) {
                e if e < 0 => Err(Error::from(e)),
                _ => Ok(context),
            }
        }
        .unwrap()
    };
    let mut decoder = context_decoder.decoder().video()?;
    let mut scaler = ffmpeg_next::software::scaling::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::RGB24,
        decoder.width(),
        decoder.height(),
        Flags::BILINEAR,
    )?;
    for (stream, packet) in ictx.packets() {
        if stream.index() == video_stream_index {
            decoder.send_packet(&packet)?;
            let mut decoded = Video::empty();
            if decoder.receive_frame(&mut decoded).is_ok() {
                let mut rgb_frame = Video::empty();
                scaler.run(&decoded, &mut rgb_frame)?;
                return Ok(rgb_frame);
            }
        }
    }
    return Err(anyhow!("Failed to find image!"));
}
