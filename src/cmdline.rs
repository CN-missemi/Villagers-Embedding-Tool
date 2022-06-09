use clap::Parser;
#[derive(Parser, Debug)]
#[clap(version, about, long_about = None, before_help = "Villager's Embedding Tools\n农民压制工具升级版：村民压制工具")]
pub struct InputArg {
    #[clap(short, long, default_value_t = 1000, help = "每轮所渲染的帧数")]
    pub chunk_size: u32,
    #[clap(
        short,
        long,
        // default_value = "lec.mp4",
        help = "输入视频文件名 (ffmpeg所支持的格式)"
    )]
    pub input: String,
    #[clap(
        short,
        long,
        default_value = "output1.mp4",
        help = "输出视频文件名 (ffmpeg所支持的格式)"
    )]
    pub output: String,
    #[clap(
        short,
        long,
        default_value = "subtitle-images",
        help = "字幕图片文件夹"
    )]
    pub subtitle_files: String,
    #[clap(short, long, default_value = "veryfast", help = "libx264编码预设")]
    pub x264_preset: String,
    #[clap(
        short,
        long,
        default_value_t = 40,
        help = "主字幕底边距离视频底部的距离"
    )]
    pub bottom_offset: u32,
    #[clap(
        short,
        long,
        default_value_t = 40,
        help = "副字幕顶边距离视频顶部的距离"
    )]
    pub top_offset: u32,
    #[clap(
        short,
        long,
        default_value_t = num_cpus::get() as u32,
        help = "编码, 解码, 渲染时所使用的并行线程数"
    )]
    pub worker_count: u32,
    #[clap(short, long, help = "调试模式(打印更多日志)")]
    pub debug: bool,
    #[clap(
        long,
        help = "视频输出码率，默认使用解码器码率"
    )]
    pub bitrate: Option<usize>,
}
