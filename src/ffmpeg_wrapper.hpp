#ifndef FFMPEG_WRAPPER_HPP
#define FFMPEG_WRAPPER_HPP

#include <string>
#include <vector>
#include <map>
#include <optional>
#include <memory>
#include "bitmap_processor.hpp"

extern "C" {
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>
#include <libavutil/avutil.h>
#include <libavutil/imgutils.h>
#include <libavutil/pixfmt.h>
}

struct VideoInfo {
    int width = 0;
    int height = 0;
    double fps = 0.0;
    double start_time = 0.0;  // ffprobe の start_time に相当
    AVRational time_base;
    AVRational sample_aspect_ratio;  // SAR (Sample Aspect Ratio)
    bool is_interlaced = false;  // インターレース判定
};

struct SubtitleFrame {
    BitmapData* bitmap = nullptr;  // RGBA ビットマップデータ（AVFrame の代わり）
    int64_t pts = 0;  // プレゼンテーションタイムスタンプ
    double timestamp = 0.0;  // 秒単位のタイムスタンプ（パケットのPTS）
    double start_time = 0.0;  // 表示開始時間（秒単位、パケットのPTS + start_display_time）
    double end_time = 0.0;  // 表示終了時間（秒単位、パケットのPTS + end_display_time）
    int x = 0;  // 字幕の X 座標
    int y = 0;  // 字幕の Y 座標
};

class FFmpegWrapper {
public:
    FFmpegWrapper();
    ~FFmpegWrapper();
    
    // ファイルを開く
    bool open_file(const std::string& filename, 
                   std::optional<double> ss = std::nullopt,
                   std::optional<double> to = std::nullopt);
    
    // 動画情報を取得
    VideoInfo get_video_info() const;
    
    // libaribcaption デコーダーを初期化
    bool init_decoder(const std::map<std::string, std::string>& libaribcaption_opts);
    
    // 次の字幕フレームを取得
    bool get_next_subtitle_frame(SubtitleFrame& subtitle_frame);
    
    // 字幕ストリームのインデックスを取得
    int get_subtitle_stream_index() const { return subtitle_stream_index_; }
    
    // リソースを解放
    void close();
    
    // デバッグモードを設定
    void set_debug(bool debug);

private:
    bool debug_ = false;  // デバッグモード
    AVFormatContext* format_ctx_ = nullptr;
    AVCodecContext* codec_ctx_ = nullptr;
    const AVCodec* codec_ = nullptr;
    int subtitle_stream_index_ = -1;
    int video_stream_index_ = -1;  // 動画ストリームのインデックス（シーク用）
    VideoInfo video_info_;
    bool user_fps_set_ = false;
    double user_fps_ = 0.0;
    int canvas_width_ = 0;  // canvas_size の幅
    int canvas_height_ = 0;  // canvas_size の高さ
    std::optional<double> ss_;  // シーク開始時刻（秒単位）
    std::optional<double> to_;  // シーク終了時刻（秒単位）
    
    // デコーダーオプションを設定
    
    // タイムベースから秒に変換
    double pts_to_seconds(int64_t pts, const AVRational& time_base) const;
};

#endif // FFMPEG_WRAPPER_HPP

