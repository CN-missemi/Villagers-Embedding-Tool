use ffmpeg_next::frame::Video;
use log::info;
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};

use crate::render::RenderData;
use anyhow::anyhow;
pub struct SubtitleEmbedder<'a> {
    render_data: &'a Vec<RenderData>,
    // begin from 0
    // next_first_flap: u64,
    buf: Vec<Video>,
    buffer_size: usize,
    top_offset: u32,
    bottom_offset: u32,
}

impl<'a> SubtitleEmbedder<'a> {
    pub fn get_buf(&mut self) -> &mut Vec<Video> {
        &mut self.buf
    }
    pub fn new(
        render_data: &'a Vec<RenderData>,
        buffer_size: usize,
        // worker_count: u32,
        top_offset: u32,
        bottom_offset: u32,
    ) -> Self {
        Self {
            // next_first_flap: 0,
            render_data,
            buf: {
                let mut v = vec![];
                v.reserve(buffer_size);
                v
            },
            buffer_size,
            top_offset,
            bottom_offset,
        }
    }
    pub fn send_frame(&mut self, frame: Video) -> anyhow::Result<bool> {
        if self.buf.len() >= self.buffer_size {
            return Err(anyhow!("Buffer is full!"));
        }
        self.buf.push(frame);
        if self.buf.len() == self.buffer_size {
            return Ok(true);
        } else {
            return Ok(false);
        }
    }
    // frame starts from 1
    pub fn embed(&mut self, frame_begin: i64, frame_end: i64) -> anyhow::Result<()> {
        info!("Embedding frame [{}, {}]", frame_begin, frame_end);
        if self.buf.len() as i64 != frame_end - frame_begin + 1 {
            return Err(anyhow!(
                "Invalid frame range! Expected {}, received {} to {}",
                self.buf.len(),
                frame_begin,
                frame_end
            ));
        }
        let frame_ref = &mut self.buf[..];
        // info!("self renderdata length = {}", self.render_data.len());
        let renderdata_ref =
            &self.render_data[(frame_begin - 1) as usize..=(frame_end - 1) as usize];
        let top_offset = self.top_offset;
        let bottom_offset = self.bottom_offset;
        // info!("top: {}, bottom: {}", top_offset, bottom_offset);
        frame_ref
            .into_par_iter()
            .zip(renderdata_ref)
            .for_each(move |(frame, render_data)| {
                unsafe {
                    let ptr_ref = *frame.as_ptr();
                    let frame_height = ptr_ref.height;
                    let frame_width = ptr_ref.width;
                    let data_ref = &mut frame.data_mut(0);
                    // info!("Main height: {}, width: {}", frame_height, frame_width);
                    if let Some(major) = &render_data.major {
                        let ExtractResult {
                            width,
                            height,
                            data,
                            linesize,
                        } = extract_things(&*major.image);
                        // info!("major height: {}, width: {}", height, width);
                        raw_embed(
                            data_ref,
                            frame_width,
                            frame_height,
                            ptr_ref.linesize[0],
                            data,
                            width,
                            height,
                            linesize,
                            Offset::MajorBottom(bottom_offset),
                        );
                    }
                    if let Some(minor) = &render_data.minor {
                        let ExtractResult {
                            width,
                            height,
                            data,
                            linesize,
                        } = extract_things(&*minor.image);
                        // info!("minor height: {}, width: {}", height, width);
                        raw_embed(
                            data_ref,
                            frame_width,
                            frame_height,
                            ptr_ref.linesize[0],
                            data,
                            width,
                            height,
                            linesize,
                            Offset::MinorTop(top_offset),
                        );
                    }

                    // let lurow = if render_data.major
                }
            });
        return Ok(());
        // todo!();
    }
    pub fn finish(&mut self) {
        self.buf.clear();
        self.buf.reserve(self.buffer_size);
    }
}

pub(crate) enum Offset {
    MajorBottom(u32),
    MinorTop(u32),
}

#[inline]
pub(crate) fn raw_embed(
    src_img: &mut [u8],
    img_colc: i32,
    img_rowc: i32,
    img_linesize: i32,
    subtitle_img: &[u8],
    colc: i32,
    rowc: i32,
    linesize: i32,
    offset: Offset,
) {
    let lurow = match offset {
        Offset::MajorBottom(v) => img_rowc - v as i32 - rowc,
        Offset::MinorTop(v) => v as i32,
    };
    let lucol = (img_colc - colc) / 2;
    /*
    i行j列像素(i,j) (从1开始)
    */
    // info!("lurow: {} lucol: {}", lurow, lucol);
    // info!(
    //     "img_colc: {} img_rowc: {} colc: {} rowc: {}",
    //     img_colc, img_rowc, colc, rowc
    // );

    for r in 0..rowc {
        for c in 0..colc {
            let img_base = ((r + lurow) * img_linesize + (c + lucol) * 3) as usize;
            let subtitle_base = (r * linesize + 3 * c) as usize;
            // 背景色半透明
            if (subtitle_img[subtitle_base]
                | subtitle_img[subtitle_base + 1]
                | subtitle_img[subtitle_base + 2])
                == 0
            {
                for x in 0..3 {
                    src_img[img_base + x] = ((subtitle_img[subtitle_base + x] as u16
                        + src_img[img_base + x] as u16)
                        / 2) as u8;
                }
            } else {
                //非背景色，不透明
                for x in 0..3 {
                    src_img[img_base + x] = subtitle_img[subtitle_base + x];
                }
            }
        }
    }
}
struct ExtractResult<'a> {
    width: i32,
    height: i32,
    data: &'a [u8],
    linesize: i32,
}
#[inline]
unsafe fn extract_things<'a>(frame: &'a Video) -> ExtractResult<'a> {
    let data_ref = *frame.as_ptr();
    let width = data_ref.width;
    let height = data_ref.height;
    let linesize = data_ref.linesize[0];
    let data = frame.data(0);
    return ExtractResult {
        width,
        height,
        data,
        linesize,
    };
}
