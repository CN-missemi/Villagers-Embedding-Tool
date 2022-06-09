use std::path::PathBuf;

use ::log::{debug, info};
use anyhow::anyhow;
use flexi_logger::{opt_format, Logger};
use rayon::ThreadPoolBuilder;

use crate::{
    cmdline::InputArg,
    embedder::SubtitleEmbedder,
    // image::read_image,
    render::{init_render_data, RenderData},
    subtitle::load_subtitles,
};
use clap::StructOpt;
use ffmpeg_next::{
    codec::{self, Context},
    encoder,
    format::{context::Output, input, output, Pixel},
    frame::Video,
    log,
    software::scaling::Flags,
    threading::Config,
    Dictionary, Error, Packet,
};
use ffmpeg_sys_next::{
    av_frame_get_buffer, avcodec_alloc_context3, avcodec_parameters_from_context,
    avcodec_parameters_to_context, AVRational,
};

mod cmdline;
mod embedder;
mod image;
mod render;
mod subtitle;

fn main() -> anyhow::Result<()> {
    log::set_level(log::Level::Info);
    ffmpeg_next::init().unwrap();
    let arg = InputArg::parse();
    Logger::try_with_str(if arg.debug { "debug" } else { "info" })
        .unwrap()
        .format(opt_format)
        .log_to_stdout()
        .start()
        .expect("Failed to start logger!");
    debug!("{:?}", arg);
    // {
    //     let mut src = read_image(&PathBuf::from("./images/out001.png")).unwrap();
    //     let sub = read_image(&PathBuf::from(
    //         "./subtitle-images/major-subtitle-1-1-100.png",
    //     ))
    //     .unwrap();
    //     let wid = src.width() as _;
    //     let hei = src.height() as _;
    //     let ls = unsafe { (*src.as_ptr()).linesize[0] };
    //     let subls = unsafe { (*sub.as_ptr()).linesize[0] };

    //     raw_embed(
    //         src.data_mut(0),
    //         wid,
    //         hei,
    //         ls,
    //         sub.data(0),
    //         sub.width() as _,
    //         sub.height() as _,
    //         subls,
    //         embedder::Offset::MajorBottom(50),
    //     );
    //     save_file(&src, 1).unwrap();
    //     // std::os::raw::sy
    //     panic!("qaq");
    // }
    let mut input_ctx =
        input(&arg.input).map_err(|e| anyhow!("Failed to open video file: {}", e))?;
    let mut output_ctx =
        output(&arg.output).map_err(|e| anyhow!("Failed to open output video file: {}", e))?;
    let input_video = input_ctx
        .streams()
        .best(ffmpeg_next::media::Type::Video)
        .ok_or(anyhow!("Failed to find video stream"))?;

    let video_stream_index = input_video.index();
    for (idx, input_stream) in input_ctx.streams().enumerate() {
        debug!(
            "Add stream {}, type {:?}",
            idx,
            input_stream.codec().medium()
        );
        let codec_id = if idx == video_stream_index {
            codec::Id::H264
        } else {
            input_stream.codec().id()
        };
        debug!("Add stream {}, id {}", idx, codec_id.name());
        let mut output_stream = output_ctx
            .add_stream(encoder::find(codec_id))
            .map_err(|e| anyhow!("Failed to add stream: {}", e))?;
        output_stream.set_parameters(input_stream.parameters());
        debug!("Stream parameters: {:#?}", unsafe {
            let v = *input_stream.as_ptr();
            v
        });
        // output_stream.set_metadata(input_stream.metadata().to_owned());
        output_stream.set_time_base(input_stream.time_base());
        unsafe {
            (*output_stream.parameters().as_mut_ptr()).codec_tag = 0;
        }
    }
    info!("Video stream index: {}", video_stream_index);
    let output_video = output_ctx.stream(video_stream_index).unwrap();

    info!("Input video codec: {:#?}", input_video.codec().id());
    info!("Output video codec: {:#?}", output_video.codec().id());

    // avcodec_paramete
    // let context_decoder = codec::context::Context::from_parameters()?;
    let context_decoder = {
        let parameters = input_video.parameters();
        let mut context = codec::context::Context::new();

        unsafe {
            match avcodec_parameters_to_context(context.as_mut_ptr(), parameters.as_ptr()) {
                e if e < 0 => Err(Error::from(e)),
                _ => Ok(context),
            }
        }
        .unwrap()
    };
    // let context_encoder = codec::context::Context::new();
    let libx264 = codec::encoder::find_by_name("libx264").expect("Missing libx264 encoder!");

    unsafe {
        let v = *libx264.as_ptr();
        debug!("Codec: {:#?}", v);
    }
    let mut decoder = context_decoder.clone().decoder().video()?;
    // let mut context_encoder = codec::context::Context::from_parameters(output_video.parameters())?;
    let mut context_encoder =
        unsafe { Context::wrap(avcodec_alloc_context3(libx264.as_ptr()), None) };
    // context_encoder.set_flags(ffmpeg_next::codec::Flags::GLOBAL_HEADER);
    // let fps = input_video.avg_frame_rate();
    // context_encoder.set
    let avg_fps = input_video.avg_frame_rate();
    let output_bitrate = arg.bitrate.unwrap_or(decoder.bit_rate());
    {
        info!(
            "Video shape (width, height) = ({}, {}), FPS = {}, total frames: {}",
            decoder.width(),
            decoder.height(),
            input_video.avg_frame_rate(),
            input_video.frames()
        );
        info!("Input file: {}", &arg.input);
        info!("Output file: {}", &arg.output);
        info!("Chunk size: {}", &arg.chunk_size);
        info!("Worker count: {}", &arg.worker_count);
        info!("Bit rate: {} kb/s", output_bitrate);
    }
    // let fps = input_video.avg_frame_rate();
    unsafe {
        let input_timebase = input_ctx.stream(video_stream_index).unwrap().time_base();

        let time_base = AVRational {
            num: input_timebase.numerator(),
            den: input_timebase.denominator(),
        };
        let time_base_inv = AVRational {
            num: input_timebase.denominator(),
            den: input_timebase.numerator(),
        };
        debug!(
            "Initial context pointer: {:?}",
            context_encoder.as_mut_ptr()
        );
        let mut context = *context_encoder.as_mut_ptr();
        let codec = *libx264.as_ptr();
        let decoder_ref = *decoder.as_ptr();
        context.codec_id = codec.id;
        context.codec_type = codec.type_;
        context.width = decoder.width() as i32;
        context.height = decoder.height() as i32;
        context.pix_fmt = decoder_ref.pix_fmt;
        context.bit_rate = decoder_ref.bit_rate;
        context.framerate = time_base;
        context.time_base = time_base_inv;
        context.gop_size = decoder_ref.gop_size;
        context.qmax = decoder_ref.qmax;
        context.qmin = decoder_ref.qmin;
        context.max_b_frames = decoder_ref.max_b_frames;
        // context.pkt_timebase
        debug!("Context encoder: {:#?}", context);
    }
    // avcodec_open2(avctx, codec, options)
    debug!("Context medium: {:?}", context_encoder.medium());
    let mut video_encoder = context_encoder.encoder().video().unwrap();

    video_encoder.set_height(decoder.height());
    video_encoder.set_width(decoder.width());
    video_encoder.set_aspect_ratio(decoder.aspect_ratio());
    video_encoder.set_format(decoder.format());
    video_encoder.set_frame_rate(Some(input_video.avg_frame_rate()));
    video_encoder.set_time_base(input_video.time_base());
    video_encoder.set_bit_rate(output_bitrate);
    unsafe {
        let decoder_ref = *decoder.as_ptr();
        video_encoder.set_gop(decoder_ref.gop_size as u32);
        video_encoder.set_qmax(decoder_ref.qmax as _);
        video_encoder.set_qmin(decoder_ref.qmin as _);
        video_encoder.set_max_b_frames(decoder_ref.max_b_frames as _);
    }
    unsafe {
        let val_ref = *video_encoder.as_ptr();
        debug!("Before open, context: {:#?}", val_ref);
        debug!("Pointer: {:?}", video_encoder.as_ptr());
    }
    let mut video_encoder = video_encoder.open_as_with(libx264, {
        let mut dict = Dictionary::new();
        dict.set("preset", &arg.x264_preset);
        dict.set("tune", "zerolatency");
        dict.set("profile", "main");
        dict
    })?;

    unsafe {
        let mut stream = output_ctx.stream_mut(video_stream_index).unwrap();
        let stream_ref = *stream.as_mut_ptr();
        let code = avcodec_parameters_from_context(stream_ref.codecpar, video_encoder.as_ptr());
        if code != 0 {
            return Err(anyhow!("Failed to copy parameters from context: {}", code));
        }
    }

    output_ctx
        .stream_mut(video_stream_index)
        .unwrap()
        .set_parameters(&video_encoder);

    output_ctx.write_header()?;

    let subtitles = load_subtitles(&PathBuf::from(&arg.subtitle_files))
        .map_err(|e| anyhow!("Failed to read subtitles: {}\n", e))?;
    info!("{} subtitles loaded.", subtitles.len());

    let mut render_data = Vec::<RenderData>::new();
    init_render_data(&mut render_data, &subtitles, input_video, &arg)?;
    info!("Render data length: {}", render_data.len());
    let mut scaler_input = ffmpeg_next::software::scaling::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::RGB24,
        decoder.width(),
        decoder.height(),
        Flags::BILINEAR,
    )?;
    let mut scaler_output = ffmpeg_next::software::scaling::Context::get(
        Pixel::RGB24,
        decoder.width(),
        decoder.height(),
        video_encoder.format(),
        video_encoder.width(),
        video_encoder.height(),
        Flags::BILINEAR,
    )?;

    ffmpeg_next::format::context::output::dump(&output_ctx, 0, Some(&arg.output));
    ThreadPoolBuilder::new()
        .num_threads(arg.worker_count as usize)
        .build_global()
        .unwrap();
    info!("Rayon threadpool initialized.");
    let mut embedder = SubtitleEmbedder::new(
        &render_data,
        arg.chunk_size as usize,
        arg.top_offset as u32,
        arg.bottom_offset as u32,
    );
    let mut output_frame_idx: i64 = 0;
    let mut input_frame_idx: i64 = 0;
    let threading_config = Config {
        kind: ffmpeg_next::threading::Type::Frame,
        safe: true,
        count: arg.worker_count as usize,
    };
    video_encoder.set_threading(threading_config.clone());
    decoder.set_threading(threading_config.clone());
    let decoder_timebase = input_ctx.stream(video_stream_index).unwrap().time_base();
    let output_timebase = output_ctx.stream(video_stream_index).unwrap().time_base();
    let mut start_frame: i64 = 1;
    let mut output_packet = Packet::empty();

    let mut write_output = |embedder: &mut SubtitleEmbedder,
                            output_ctx: &mut Output,
                            start_frame: i64,
                            end_frame: i64|
     -> anyhow::Result<()> {
        let embed_start = std::time::Instant::now();
        let video_sec = (end_frame - start_frame + 1) as f64
            / (avg_fps.numerator() as f64 / avg_fps.denominator() as f64);
        info!(
            "Starting embedding for flap {} to {}, video secs {}",
            start_frame, end_frame, video_sec
        );
        embedder
            .embed(start_frame, end_frame)
            .expect("Failed to perform embedding");
        info!("Embedding done for flap {} to {}", start_frame, end_frame);
        {
            // let curr = timestamp();
            let secs = embed_start.elapsed().as_secs_f64();
            info!(
                "Embedding speed: {:.3}x, time usage: {:.4} secs",
                video_sec / secs,
                secs,
            );
        }
        let mut output_frame = Video::empty();

        info!("Chunk encoding started.");
        let encode_start = std::time::Instant::now();
        for frame in embedder.get_buf().iter() {
            output_frame_idx += 1;
            let pts = frame.pts();
            scaler_output
                .run(&frame, &mut output_frame)
                .map_err(|e| anyhow!("Failed to run output scaler: {}", e))?;

            output_frame.set_pts(pts);
            video_encoder
                .send_frame(&output_frame)
                .map_err(|e| anyhow!("Failed to send frame to video encoder: {}", e))?;
            // 写输出
            while video_encoder.receive_packet(&mut output_packet).is_ok() {
                output_packet.set_stream(video_stream_index);
                output_packet.rescale_ts(decoder_timebase, output_timebase);
                output_packet
                    .write_interleaved(output_ctx)
                    .map_err(|e| anyhow!("Failed to write output stream: {}", e))?;
            }
        }
        {
            let secs = encode_start.elapsed().as_secs_f64();
            info!(
                "Encoding speed: {:.3}x, time usage: {:.4} secs",
                video_sec / secs,
                secs
            );
        }
        embedder.finish();
        info!("Frame done: {} to {}", start_frame, end_frame);

        return Ok(());
    };
    let mut last_decode_start = std::time::Instant::now();
    let mut curr_decoding = false;
    for (input_stream, mut input_packet) in input_ctx.packets() {
        if input_stream.index() == video_stream_index {
            decoder
                .send_packet(&input_packet)
                .map_err(|e| anyhow!("Failed to send packet to decoder: {}", e))?;
            let mut decoded = Video::empty();
            if !curr_decoding {
                info!("");
                info!("Chunk decoding started.");
                curr_decoding = true;
            }
            while decoder.receive_frame(&mut decoded).is_ok() {
                // debug!("Input frame {}, pts: {:?}", input_frame_idx, decoded.pts());
                let mut rgb_frame = Video::empty();
                unsafe {
                    rgb_frame.set_format(Pixel::RGB24);
                    rgb_frame.set_width(decoded.width());
                    rgb_frame.set_height(decoded.height());
                    let err = av_frame_get_buffer(rgb_frame.as_mut_ptr(), 32);
                    if err != 0 {
                        let e = Error::from(err);
                        return Err(anyhow!(
                            "Failed to get buffer for rgb_frame: {}, {}",
                            err,
                            e
                        ));
                    }
                }
                scaler_input
                    .run(&decoded, &mut rgb_frame)
                    .map_err(|e| anyhow!("Failed to run input scaler: {}. This should not happen, consider your memory usage.", e))?;
                rgb_frame.set_pts(decoded.pts());
                input_frame_idx += 1;
                if embedder
                    .send_frame(rgb_frame)
                    .expect("Failed to send frame to embedder")
                {
                    let video_sec = (input_frame_idx - start_frame + 1) as f64
                        / (avg_fps.numerator() as f64 / avg_fps.denominator() as f64);
                    let secs = last_decode_start.elapsed().as_secs_f64();
                    info!("Chunk decoding done.");
                    info!(
                        "Decoding speed: {:.3}x, time usage: {:.4} secs",
                        video_sec / secs,
                        secs
                    );
                    write_output(&mut embedder, &mut output_ctx, start_frame, input_frame_idx)?;
                    start_frame = input_frame_idx + 1;
                    last_decode_start = std::time::Instant::now();
                    curr_decoding = false;
                }
            }
        } else {
            let output_stream = output_ctx.stream(input_stream.index()).unwrap();
            input_packet.rescale_ts(input_stream.time_base(), output_stream.time_base());
            input_packet.set_stream(output_stream.index());
            input_packet.set_position(-1);
            input_packet
                .write_interleaved(&mut output_ctx)
                .map_err(|e| anyhow!("Failed to write packet: {}", e))?;
        }
    }
    decoder.send_eof()?;
    if !embedder.get_buf().is_empty() {
        write_output(&mut embedder, &mut output_ctx, start_frame, input_frame_idx)?;
    }

    video_encoder.send_eof()?;
    output_ctx.write_trailer()?;
    unsafe {
        let final_stream = *output_ctx.stream(video_stream_index).unwrap().as_ptr();
        debug!("Final stream: {:#?}", final_stream.nb_frames);
    }
    info!("Done!");
    return Ok(());
}

// fn save_file(frame: &Video, index: i32) -> std::result::Result<(), std::io::Error> {
//     use std::io::Write;
//     let mut file = std::fs::File::create(format!("{}.ppm", index))?;
//     file.write_all(format!("P6\n{} {}\n255\n", frame.width(), frame.height()).as_bytes())?;
//     file.write_all(frame.data(0))?;
//     Ok(())
// }
// #[inline]
// fn timestamp() -> f64 {
//     let value = std::time::SystemTime::now()
//         .duration_since(std::time::UNIX_EPOCH)
//         .unwrap()
//         .as_secs_f64();
//     return value;
// }
