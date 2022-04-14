use std::sync::Arc;

use ffmpeg_next::{frame::Video, Stream};
use log::warn;

use crate::{subtitle::{Subtitle, self}, cmdline::InputArg};


pub struct SubtitleWrapper {
    pub id: usize,
    pub image: Arc<Video>,
}

pub struct RenderData {
    pub flap: usize,
    pub major: Option<SubtitleWrapper>,
    pub minor: Option<SubtitleWrapper>,
    pub bottom_offset: usize,
    pub top_offset: usize,
}
#[inline]
pub fn init_render_data(
    render_data: &mut Vec<RenderData>,
    subtitles: &Vec<Subtitle>,
    input_video: Stream,
    arg: &InputArg,
) -> anyhow::Result<()> {
    render_data.reserve(input_video.frames() as usize);
    for i in 0..input_video.frames() {
        render_data.push(RenderData {
            flap: i as usize,
            major: None,
            minor: None,
            bottom_offset: arg.bottom_offset as usize,
            top_offset: arg.top_offset as usize,
        });
    }
    for subtitle in subtitles.iter() {
        let Subtitle {
            subtitle_type,
            begin_flap,
            end_flap,
            data,
            id,
        } = subtitle;
        for flap in &mut render_data[(*begin_flap as usize) - 1..=(*end_flap as usize) - 1] {
            match subtitle_type {
                subtitle::SubtitleType::Major => {
                    if let Some(prev) = &flap.major {
                        warn!(
                            "Conflict major subtitle: {} and {}, at flap {} (from 1), overriding",
                            prev.id,
                            id,
                            flap.flap + 1,
                        )
                    }
                    flap.major = Some(SubtitleWrapper {
                        id: *id as usize,
                        image: data.clone(),
                    });
                }
                subtitle::SubtitleType::Minor => {
                    if let Some(prev) = &flap.minor {
                        warn!(
                            "Conflict minor subtitle: {} and {}, at flap {} (from 1), overriding",
                            prev.id,
                            id,
                            flap.flap + 1
                        );
                    }
                    flap.minor = Some(SubtitleWrapper {
                        id: *id as usize,
                        image: data.clone(),
                    });
                }
            };
        }
    }

    return Ok(());
}
