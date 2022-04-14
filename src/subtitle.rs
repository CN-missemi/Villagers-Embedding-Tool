use std::{path::Path, sync::Arc};

use anyhow::anyhow;
use ffmpeg_next::frame::Video;
use log::{debug, error, info};
use regex::Regex;

use crate::image::read_image;

lazy_static::lazy_static! {
    static ref FILENAME_EXPR:Regex = Regex::new(r#"(?P<type>(major)|(minor))-subtitle-(?P<id>[0-9]+)-(?P<begin>[0-9]+)-(?P<end>[0-9]+)\.png"#).unwrap();
}
pub enum SubtitleType {
    Major,
    Minor,
}
pub struct Subtitle {
    pub subtitle_type: SubtitleType,
    pub id: u64,
    pub begin_flap: u64,
    pub end_flap: u64,
    pub data: Arc<Video>,
}

pub fn load_subtitles(root: &Path) -> anyhow::Result<Vec<Subtitle>> {
    let mut subtitles = vec![];
    for file in std::fs::read_dir(root)
        .map_err(|e| anyhow!("Failed to read directory: {}", e))?
        .into_iter()
    {
        match file {
            Ok(file) => {
                let path = file.path();
                let filename = path.file_name().unwrap().to_str().unwrap();
                debug!("Reading: {}", filename);
                if let Some(groups) = FILENAME_EXPR.captures(filename) {
                    let subtitle_type = match groups.name("type").unwrap().as_str() {
                        "major" => SubtitleType::Major,
                        "minor" => SubtitleType::Minor,
                        _ => {
                            return Err(anyhow!(
                                "Invalid subtitle type: {}",
                                groups.name("type").unwrap().as_str()
                            ));
                        }
                    };
                    let id = groups.name("id").unwrap().as_str().parse::<u64>().unwrap();
                    let begin_flap = groups
                        .name("begin")
                        .unwrap()
                        .as_str()
                        .parse::<u64>()
                        .map_err(|e| {
                            anyhow!("Invalid begin flap number for file {}, {}", filename, e)
                        })?;
                    let end_flap = groups
                        .name("end")
                        .unwrap()
                        .as_str()
                        .parse::<u64>()
                        .map_err(|e| {
                            anyhow!("Invalid end flap number for file {}, {}", filename, e)
                        })?;
                    let data =
                        read_image(&path).map_err(|e| anyhow!("Failed to read image: {}", e))?;
                    let subtitle = Subtitle {
                        subtitle_type,
                        id,
                        begin_flap,
                        end_flap,
                        data: Arc::new(data),
                    };
                    subtitles.push(subtitle);
                } else {
                    info!("Ignoring file: {}", filename);
                }
            }
            Err(e) => {
                error!("Failed to read :{}, ignoring", e);
            }
        };
    }
    return Ok(subtitles);
}
