#include "ffmpeg_wrapper.hpp"
#include "bitmap_processor.hpp"
#include <iostream>
#include <stdexcept>
#include <cstring>
#include <cstdint>
#include <climits>
#include <algorithm>

extern "C" {
#include <libavutil/error.h>
#include <libavutil/log.h>
}

// デバッグログ用マクロ
#define DEBUG_LOG(x) do { if (debug_) std::cout << x << std::endl; } while(0)

FFmpegWrapper::FFmpegWrapper() {
    // FFmpeg の初期化は不要（FFmpeg 4.0+ では自動初期化）
    // デフォルトでFFmpegのログレベルをFATALに設定（警告やエラーを非表示）
    av_log_set_level(AV_LOG_FATAL);
}

FFmpegWrapper::~FFmpegWrapper() {
    close();
}

bool FFmpegWrapper::open_file(const std::string& filename) {
    DEBUG_LOG("open_file: 開始");
    
    // ARIB 字幕が途中から始まる可能性があるため、より多くのデータを解析する
    AVDictionary* format_opts = nullptr;
    // 値が大きすぎると問題を引き起こす可能性があるため、適切な値に調整
    av_dict_set(&format_opts, "analyzeduration", "150000000", 0);  // 150M (マイクロ秒)
    av_dict_set(&format_opts, "probesize", "150000000", 0);  // 150M (バイト)
    // fflags で genpts と igndts を設定（タイムスタンプ推定を有効化）
    av_dict_set(&format_opts, "fflags", "+genpts+igndts", 0);
    
    DEBUG_LOG("open_file: フォーマットオプションを設定しました");
    
    DEBUG_LOG("open_file: avformat_open_input を呼び出し中...");
    // ファイルを開く
    int ret = avformat_open_input(&format_ctx_, filename.c_str(), nullptr, &format_opts);
    if (format_opts) {
        av_dict_free(&format_opts);
    }
    if (ret < 0) {
        char errbuf[AV_ERROR_MAX_STRING_SIZE];
        av_strerror(ret, errbuf, AV_ERROR_MAX_STRING_SIZE);
        std::cerr << "エラー: ファイルを開けませんでした: " << filename << " (" << errbuf << ")" << std::endl;
        return false;
    }
    DEBUG_LOG("open_file: ファイルを開きました");
    
    // ストリーム情報を取得
    DEBUG_LOG("open_file: ストリーム情報を取得中...");
    
    DEBUG_LOG("open_file: avformat_find_stream_info を呼び出し中...");
    // nullptr を渡してデフォルトの動作で試す
    int stream_info_ret = avformat_find_stream_info(format_ctx_, nullptr);
    if (stream_info_ret < 0) {
        char errbuf[AV_ERROR_MAX_STRING_SIZE];
        av_strerror(stream_info_ret, errbuf, AV_ERROR_MAX_STRING_SIZE);
        std::cerr << "エラー: ストリーム情報を取得できませんでした: " << errbuf << std::endl;
        return false;
    }
    DEBUG_LOG("open_file: ストリーム情報を取得しました");
    
    // 字幕ストリームを探す（ARIB 字幕）
    if (debug_) {
        std::cout << "字幕ストリームを検索中... (総ストリーム数: " << format_ctx_->nb_streams << ")" << std::endl;
    }
    subtitle_stream_index_ = -1;
    for (unsigned int i = 0; i < format_ctx_->nb_streams; i++) {
        AVStream* stream = format_ctx_->streams[i];
        if (!stream || !stream->codecpar) {
            continue;
        }
        
        if (debug_) {
            std::cout << "ストリーム " << i << ": タイプ=" << stream->codecpar->codec_type;
        }
        if (stream->codecpar->codec_type == AVMEDIA_TYPE_SUBTITLE) {
            // libaribcaption デコーダーに対応しているかチェック
            const AVCodec* codec = avcodec_find_decoder(stream->codecpar->codec_id);
            if (codec) {
                const char* codec_name = codec->name;
                if (debug_) {
                    std::cout << ", コーデック=" << codec_name;
                }
                // デコーダー名に "arib" または "libaribcaption" が含まれているかチェック
                if (strstr(codec_name, "arib") != nullptr ||
                    strstr(codec_name, "libaribcaption") != nullptr) {
                    subtitle_stream_index_ = i;
                    if (debug_) {
                        std::cout << " <- 選択";
                    }
                    break;
                }
            }
        }
        if (debug_) {
            std::cout << std::endl;
        }
    }
    
    if (subtitle_stream_index_ == -1) {
        std::cerr << "エラー: ARIB 字幕ストリームが見つかりませんでした。" << std::endl;
        return false;
    }
    if (debug_) {
        std::cout << "字幕ストリームが見つかりました: インデックス " << subtitle_stream_index_ << std::endl;
    }
    
    // 動画ストリームを探して情報を取得
    int video_stream_index = -1;
    for (unsigned int i = 0; i < format_ctx_->nb_streams; i++) {
        if (format_ctx_->streams[i]->codecpar->codec_type == AVMEDIA_TYPE_VIDEO) {
            video_stream_index = i;
            break;
        }
    }
    if (video_stream_index >= 0) {
        AVStream* video_stream = format_ctx_->streams[video_stream_index];
        video_info_.width = video_stream->codecpar->width;
        video_info_.height = video_stream->codecpar->height;
        
        // SAR (Sample Aspect Ratio) を取得
        if (video_stream->codecpar->sample_aspect_ratio.num > 0 && 
            video_stream->codecpar->sample_aspect_ratio.den > 0) {
            video_info_.sample_aspect_ratio = video_stream->codecpar->sample_aspect_ratio;
        } else if (video_stream->sample_aspect_ratio.num > 0 && 
                   video_stream->sample_aspect_ratio.den > 0) {
            video_info_.sample_aspect_ratio = video_stream->sample_aspect_ratio;
        } else {
            // デフォルトは 1:1
            video_info_.sample_aspect_ratio.num = 1;
            video_info_.sample_aspect_ratio.den = 1;
        }
        
        // フレームレートを取得
        if (video_stream->avg_frame_rate.num > 0 && video_stream->avg_frame_rate.den > 0) {
            video_info_.fps = av_q2d(video_stream->avg_frame_rate);
        } else if (video_stream->r_frame_rate.num > 0 && video_stream->r_frame_rate.den > 0) {
            video_info_.fps = av_q2d(video_stream->r_frame_rate);
        }
        
        video_info_.time_base = video_stream->time_base;
    }
    
    // start_time を取得（ffprobe -show_entries format=start_time に相当）
    if (format_ctx_->start_time != AV_NOPTS_VALUE) {
        video_info_.start_time = format_ctx_->start_time / (double)AV_TIME_BASE;
    } else {
        video_info_.start_time = 0.0;
    }
    
    return true;
}

VideoInfo FFmpegWrapper::get_video_info() const {
    VideoInfo info = video_info_;
    
    // ユーザー指定の FPS があれば使用
    if (user_fps_set_) {
        info.fps = user_fps_;
    }
    
    return info;
}

bool FFmpegWrapper::init_decoder(const std::map<std::string, std::string>& libaribcaption_opts) {
    if (subtitle_stream_index_ < 0) {
        std::cerr << "エラー: 字幕ストリームが設定されていません。" << std::endl;
        return false;
    }
    
    AVStream* stream = format_ctx_->streams[subtitle_stream_index_];
    
    // デコーダーを探す
    codec_ = avcodec_find_decoder(stream->codecpar->codec_id);
    if (!codec_) {
        std::cerr << "エラー: デコーダーが見つかりませんでした。" << std::endl;
        std::cerr << "コーデック ID: " << stream->codecpar->codec_id << std::endl;
        return false;
    }
    
    if (debug_) {
        std::cout << "デコーダー: " << codec_->name << " (" << codec_->long_name << ")" << std::endl;
    }
    // デコーダーコンテキストを作成
    codec_ctx_ = avcodec_alloc_context3(codec_);
    if (!codec_ctx_) {
        std::cerr << "エラー: デコーダーコンテキストを作成できませんでした。" << std::endl;
        return false;
    }
    
    // パラメータをコピー
    DEBUG_LOG("init_decoder: パラメータをコピー中...");
    if (avcodec_parameters_to_context(codec_ctx_, stream->codecpar) < 0) {
        std::cerr << "エラー: デコーダーパラメータをコピーできませんでした。" << std::endl;
        avcodec_free_context(&codec_ctx_);
        return false;
    }
    DEBUG_LOG("init_decoder: パラメータをコピーしました");
    
    // タイムベースを設定（字幕ストリームのタイムベースを使用）
    codec_ctx_->time_base = stream->time_base;
    DEBUG_LOG("init_decoder: タイムベースを設定しました: " << codec_ctx_->time_base.num << "/" << codec_ctx_->time_base.den);
    // libaribcaption デコーダーの場合、codec_type を確認
    // 字幕デコーダーだが、ビットマップを出力するため、特別な処理が必要
    DEBUG_LOG("init_decoder: codec_type=" << codec_ctx_->codec_type << " (AVMEDIA_TYPE_SUBTITLE=" << AVMEDIA_TYPE_SUBTITLE << ")");
    // デコーダーオプションを設定
    AVDictionary* opts_dict = nullptr;
    
    // libaribcaption デコーダーの場合、ビットマップ出力のため sub_type=bitmap を設定
    if (strstr(codec_->name, "arib") != nullptr || strstr(codec_->name, "libaribcaption") != nullptr) {
        DEBUG_LOG("libaribcaption デコーダーを検出しました");
        // libaribcaption デコーダーはビットマップを出力するため、
        // デコーダーコンテキストの pix_fmt を確認
        if (debug_) {
            std::cout << "init_decoder: pix_fmt=" << codec_ctx_->pix_fmt << std::endl;
            std::cout << "init_decoder: width=" << codec_ctx_->width << ", height=" << codec_ctx_->height << std::endl;
        }
        // sub_type を bitmap に設定（ビットマップ出力のため）
        av_dict_set(&opts_dict, "sub_type", "bitmap", 0);
        DEBUG_LOG("init_decoder: sub_type を bitmap に設定");
        
        // canvas_size を解析して保存し、opts_dict に設定
        if (libaribcaption_opts.find("canvas_size") != libaribcaption_opts.end()) {
            std::string canvas_size = libaribcaption_opts.at("canvas_size");
            size_t x_pos = canvas_size.find('x');
            if (x_pos != std::string::npos) {
                try {
                    canvas_width_ = std::stoi(canvas_size.substr(0, x_pos));
                    canvas_height_ = std::stoi(canvas_size.substr(x_pos + 1));
                    if (debug_) {
                        std::cout << "init_decoder: canvas_size を解析: " << canvas_width_ << "x" << canvas_height_ << std::endl;
                    }
                    // opts_dict に設定（ユーザー指定またはDARに基づいて計算された値）
                    av_dict_set(&opts_dict, "canvas_size", canvas_size.c_str(), 0);
                    if (debug_) {
                        std::cout << "init_decoder: canvas_size を opts_dict に設定: " << canvas_size << std::endl;
                    }
                    // デコーダーコンテキストの解像度も canvas_size に基づいて設定
                    codec_ctx_->width = canvas_width_;
                    codec_ctx_->height = canvas_height_;
                    if (debug_) {
                        std::cout << "init_decoder: デコーダーコンテキストの解像度を canvas_size に設定: " << canvas_width_ << "x" << canvas_height_ << std::endl;
                    }
                } catch (...) {
                    std::cerr << "エラー: 無効な canvas_size オプション: " << canvas_size << std::endl;
                    return false;
                }
            } else {
                std::cerr << "エラー: canvas_size の形式が不正です: " << canvas_size << std::endl;
                return false;
            }
        } else {
            // canvas_size が設定されていない場合はエラー
            // main.cpp で既に canvas_size が設定されているはずなので、ここに来ることはないはず
            std::cerr << "エラー: canvas_size が設定されていません。" << std::endl;
            return false;
        }
        // pix_fmt が設定されていない場合は明示的に設定
        if (codec_ctx_->pix_fmt == AV_PIX_FMT_NONE || codec_ctx_->pix_fmt == -1) {
            // libaribcaption デコーダーは通常 RGBA 形式でビットマップを出力
            codec_ctx_->pix_fmt = AV_PIX_FMT_RGBA;
            DEBUG_LOG("init_decoder: pix_fmt を AV_PIX_FMT_RGBA に設定しました");
        }
        
        // libaribcaption デコーダーは字幕デコーダーだが、ビットマップを出力するため、
        // デコーダーコンテキストの extradata を確認
        if (codec_ctx_->extradata && codec_ctx_->extradata_size > 0) {
            if (debug_) {
                std::cout << "init_decoder: extradata サイズ=" << codec_ctx_->extradata_size << std::endl;
            }
        } else {
            DEBUG_LOG("init_decoder: extradata なし");
        }
    }
    
    // ユーザー指定のオプションを設定（sub_type と canvas_size は既に設定済みなのでスキップ）
    for (const auto& [key, value] : libaribcaption_opts) {
        // sub_type は内部で自動設定するため、ユーザー指定があっても無視
        if (key == "sub_type") {
            DEBUG_LOG("init_decoder: sub_type は内部で自動設定されるため、ユーザー指定を無視します");
            continue;
        }
        // canvas_size は既に opts_dict に設定済みなのでスキップ（重複を避ける）
        if (key == "canvas_size") {
            continue;
        }
        av_dict_set(&opts_dict, key.c_str(), value.c_str(), 0);
        if (debug_) {
            std::cout << "init_decoder: オプションを設定: " << key << "=" << value << std::endl;
        }
    }
    
    // デコーダーを開く（オプションを渡す）
    DEBUG_LOG("init_decoder: avcodec_open2 を呼び出し中...");
    DEBUG_LOG("init_decoder: オプションを渡す前の状態 - pix_fmt=" << codec_ctx_->pix_fmt
              << ", width=" << codec_ctx_->width << ", height=" << codec_ctx_->height);
    
    // デコーダーを開く前に、デコーダーコンテキストの状態を再確認
    DEBUG_LOG("init_decoder: avcodec_open2 を呼び出す直前の状態:");
    if (debug_) {
        std::cout << "  - codec_id: " << codec_ctx_->codec_id << std::endl;
        std::cout << "  - codec_type: " << codec_ctx_->codec_type << std::endl;
        std::cout << "  - pix_fmt: " << codec_ctx_->pix_fmt << std::endl;
        std::cout << "  - width: " << codec_ctx_->width << ", height: " << codec_ctx_->height << std::endl;
        std::cout << "  - time_base: " << codec_ctx_->time_base.num << "/" << codec_ctx_->time_base.den << std::endl;
    }
    int ret = avcodec_open2(codec_ctx_, codec_, &opts_dict);
    if (opts_dict) {
        av_dict_free(&opts_dict);
    }
    if (ret < 0) {
        char errbuf[AV_ERROR_MAX_STRING_SIZE];
        av_strerror(ret, errbuf, AV_ERROR_MAX_STRING_SIZE);
        std::cerr << "エラー: デコーダーを開けませんでした: " << errbuf << std::endl;
        avcodec_free_context(&codec_ctx_);
        return false;
    }
    DEBUG_LOG("init_decoder: デコーダーを開きました (戻り値: " << ret << ")");
    // デコーダーを開いた後の状態を確認
    DEBUG_LOG("init_decoder: avcodec_open2 後の状態:");
    if (debug_) {
        std::cout << "  - codec_id: " << codec_ctx_->codec_id << std::endl;
        std::cout << "  - codec_type: " << codec_ctx_->codec_type << std::endl;
        std::cout << "  - pix_fmt: " << codec_ctx_->pix_fmt << std::endl;
        std::cout << "  - width: " << codec_ctx_->width << ", height: " << codec_ctx_->height << std::endl;
        DEBUG_LOG("init_decoder: デコーダーコンテキストの状態確認 - pix_fmt=" << codec_ctx_->pix_fmt
                  << ", width=" << codec_ctx_->width << ", height=" << codec_ctx_->height);
        DEBUG_LOG("init_decoder: codec_id=" << codec_ctx_->codec_id << ", codec_type=" << codec_ctx_->codec_type);
    }
    // libaribcaption デコーダーの場合、デコーダーが内部状態を正しく設定しているか確認
    if (strstr(codec_->name, "arib") != nullptr || strstr(codec_->name, "libaribcaption") != nullptr) {
        // デコーダーの capabilities を確認
        if (debug_) {
            std::cout << "init_decoder: デコーダーの capabilities - "
                      << "AV_CODEC_CAP_DELAY=" << (codec_->capabilities & AV_CODEC_CAP_DELAY ? "yes" : "no")
                      << ", AV_CODEC_CAP_DR1=" << (codec_->capabilities & AV_CODEC_CAP_DR1 ? "yes" : "no") << std::endl;
        }
    }
    return true;
}

bool FFmpegWrapper::get_next_subtitle_frame(SubtitleFrame& subtitle_frame) {
    if (!codec_ctx_ || !format_ctx_) {
        std::cerr << "エラー: デコーダーまたはフォーマットコンテキストが初期化されていません。" << std::endl;
        return false;
    }
    
    // フレームを初期化
    subtitle_frame.bitmap = nullptr;
    subtitle_frame.pts = 0;
    subtitle_frame.timestamp = 0.0;
    subtitle_frame.start_time = 0.0;
    subtitle_frame.end_time = 0.0;
    subtitle_frame.x = 0;
    subtitle_frame.y = 0;
    
    AVPacket* packet = av_packet_alloc();
    if (!packet) {
        return false;
    }
    
    while (av_read_frame(format_ctx_, packet) >= 0) {
        if (packet->stream_index == subtitle_stream_index_) {
            DEBUG_LOG("get_next_subtitle_frame: 字幕パケットを検出、デコーダーに送信中...");
            if (debug_) {
                std::cout << "get_next_subtitle_frame: パケットサイズ=" << packet->size
                          << ", pts=" << packet->pts << ", dts=" << packet->dts << std::endl;
            }
            // 字幕デコーダーは avcodec_decode_subtitle2 API を使用
            AVSubtitle subtitle;
            memset(&subtitle, 0, sizeof(subtitle));
            int got_subtitle = 0;
            
            int ret = avcodec_decode_subtitle2(codec_ctx_, &subtitle, &got_subtitle, packet);
            if (debug_) {
                std::cout << "get_next_subtitle_frame: avcodec_decode_subtitle2 の戻り値: " << ret
                          << ", got_subtitle: " << got_subtitle << std::endl;
            }
            
            if (ret < 0) {
                char errbuf[AV_ERROR_MAX_STRING_SIZE];
                av_strerror(ret, errbuf, AV_ERROR_MAX_STRING_SIZE);
                std::cerr << "警告: 字幕デコードエラー: " << errbuf << std::endl;
                av_packet_unref(packet);
                continue;
            }
            
            if (got_subtitle) {
                DEBUG_LOG("get_next_subtitle_frame: 字幕を取得しました - num_rects=" << subtitle.num_rects);
                
                // ビットマップデータを RGBA 形式に変換
                // 複数の rect がある場合は、すべてを1つのビットマップに合成
                if (subtitle.num_rects > 0) {
                    // すべての rect の境界を計算
                    int min_x = INT_MAX, min_y = INT_MAX;
                    int max_x = INT_MIN, max_y = INT_MIN;
                    bool has_bitmap = false;
                    
                    for (unsigned int i = 0; i < subtitle.num_rects; i++) {
                        if (subtitle.rects[i]->type == SUBTITLE_BITMAP) {
                            has_bitmap = true;
                            AVSubtitleRect* rect = subtitle.rects[i];
                            min_x = std::min(min_x, rect->x);
                            min_y = std::min(min_y, rect->y);
                            max_x = std::max(max_x, rect->x + rect->w);
                            max_y = std::max(max_y, rect->y + rect->h);
                        }
                    }
                    if (!has_bitmap) {
                        DEBUG_LOG("get_next_subtitle_frame: ビットマップ字幕がありません");
                        avsubtitle_free(&subtitle);
                        av_packet_unref(packet);
                        continue;
                    }
                    
                    // 合成ビットマップのサイズを計算
                    int composite_width = max_x - min_x;
                    int composite_height = max_y - min_y;
                    
                    if (debug_) {
                        std::cout << "get_next_subtitle_frame: 合成ビットマップ - 幅=" << composite_width
                                  << ", 高さ=" << composite_height << ", rect数=" << subtitle.num_rects << std::endl;
                    }
                    
                    // BitmapData を作成
                    BitmapData* bitmap = new BitmapData();
                    bitmap->width = composite_width;
                    bitmap->height = composite_height;
                    bitmap->stride = composite_width * 4;  // RGBA 形式なので 4 バイト/ピクセル
                    
                    // 透明な背景で初期化
                    bitmap->data.resize(bitmap->width * bitmap->height * 4, 0);
                    
                    // 各 rect を合成ビットマップに描画
                    for (unsigned int i = 0; i < subtitle.num_rects; i++) {
                        AVSubtitleRect* rect = subtitle.rects[i];
                        if (rect->type != SUBTITLE_BITMAP) {
                            continue;
                        }
                        
                        // パレットデータの確認
                        if (!rect->data[0] || !rect->data[1]) {
                            std::cerr << "警告: パレットデータが不完全です (rect " << i << ")" << std::endl;
                            continue;
                        }
                        
                        // デバッグ情報
                        if (debug_) {
                            std::cout << "get_next_subtitle_frame: rect " << i << " - サイズ=" << rect->w
                                      << "x" << rect->h << ", 位置=" << rect->x << "," << rect->y << std::endl;
                        }
                        
                        // ビットマップをRGBA形式に変換
                        // FFmpeg のパレットは data[1] に ARGB 形式（リトルエンディアン）で格納されている
                        uint32_t* palette = (uint32_t*)rect->data[1];  // パレットテーブル（ARGB 形式）
                        uint8_t* indices = rect->data[0];  // パレットインデックス
                        
                        // rect の位置を合成ビットマップの座標系に変換
                        int dest_x = rect->x - min_x;
                        int dest_y = rect->y - min_y;
                        
                        // ビットマップデータを合成ビットマップにコピー（スケーリングなし）
                        for (int y = 0; y < rect->h; y++) {
                            for (int x = 0; x < rect->w; x++) {
                                // パレットインデックスを取得
                                uint8_t index = indices[y * rect->linesize[0] + x];
                                
                                // パレットから ARGB 値を取得（リトルエンディアン）
                                uint32_t argb = palette[index];
                                
                                // ARGB から RGBA に変換
                                uint8_t r = (argb >> 16) & 0xFF;
                                uint8_t g = (argb >> 8) & 0xFF;
                                uint8_t b = (argb >> 0) & 0xFF;
                                uint8_t a = (argb >> 24) & 0xFF;
                                
                                // 合成ビットマップの座標を計算
                                int comp_x = dest_x + x;
                                int comp_y = dest_y + y;
                                
                                if (comp_x >= 0 && comp_x < composite_width && 
                                    comp_y >= 0 && comp_y < composite_height) {
                                    // アルファブレンディング（既存のピクセルと合成）
                                    int offset = (comp_y * composite_width + comp_x) * 4;
                                    if (a > 0) {
                                        // アルファ値が0でない場合のみ描画
                                        if (a == 255 || bitmap->data[offset + 3] == 0) {
                                            // 完全に不透明または背景が透明な場合は直接コピー
                                            bitmap->data[offset + 0] = r;
                                            bitmap->data[offset + 1] = g;
                                            bitmap->data[offset + 2] = b;
                                            bitmap->data[offset + 3] = a;
                                        } else {
                                            // アルファブレンディング
                                            float alpha = a / 255.0f;
                                            float inv_alpha = 1.0f - alpha;
                                            bitmap->data[offset + 0] = (uint8_t)(r * alpha + bitmap->data[offset + 0] * inv_alpha);
                                            bitmap->data[offset + 1] = (uint8_t)(g * alpha + bitmap->data[offset + 1] * inv_alpha);
                                            bitmap->data[offset + 2] = (uint8_t)(b * alpha + bitmap->data[offset + 2] * inv_alpha);
                                            bitmap->data[offset + 3] = (uint8_t)(a + bitmap->data[offset + 3] * inv_alpha);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    // PTS を設定
                    if (packet->pts != AV_NOPTS_VALUE) {
                        subtitle_frame.pts = packet->pts;
                    } else {
                        subtitle_frame.pts = subtitle.pts;
                    }
                    
                    subtitle_frame.bitmap = bitmap;
                    subtitle_frame.x = min_x;  // 合成ビットマップの左上座標
                    subtitle_frame.y = min_y;
                    
                    // タイムスタンプを計算（パケットのPTS + start_display_time/end_display_time）
                    AVStream* stream = format_ctx_->streams[subtitle_stream_index_];
                    double base_timestamp = 0.0;
                    if (subtitle_frame.pts != AV_NOPTS_VALUE) {
                        base_timestamp = pts_to_seconds(subtitle_frame.pts, stream->time_base);
                        subtitle_frame.timestamp = base_timestamp;
                    } else {
                        subtitle_frame.timestamp = 0.0;
                        base_timestamp = 0.0;
                    }
                    
                    // start_display_time と end_display_time を使用して表示時間を計算
                    // これらはミリ秒単位で、パケットのPTSを基準にした相対時間
                    // 0xFFFFFFFF の場合は無効
                    if (subtitle.start_display_time != 0xFFFFFFFF && subtitle.end_display_time != 0xFFFFFFFF) {
                        subtitle_frame.start_time = base_timestamp + (subtitle.start_display_time / 1000.0);
                        subtitle_frame.end_time = base_timestamp + (subtitle.end_display_time / 1000.0);
                        if (debug_) {
                            std::cout << "get_next_subtitle_frame: 表示時間 - start_display_time=" << subtitle.start_display_time
                                      << "ms, end_display_time=" << subtitle.end_display_time << "ms" << std::endl;
                        }
                        if (debug_) {
                            std::cout << "get_next_subtitle_frame: タイムスタンプ - base=" << base_timestamp
                                      << "s, start=" << subtitle_frame.start_time << "s, end=" << subtitle_frame.end_time << "s" << std::endl;
                        }
                    } else {
                        // start_display_time または end_display_time が無効な場合、パケットのPTSを使用
                        subtitle_frame.start_time = base_timestamp;
                        subtitle_frame.end_time = base_timestamp;
                        DEBUG_LOG("get_next_subtitle_frame: 警告: start_display_time または end_display_time が無効です。パケットのPTSを使用します。");
                    }
                    
                    if (debug_) {
                        std::cout << "get_next_subtitle_frame: RGBA ビットマップを作成しました - 幅=" << bitmap->width 
                                  << ", 高さ=" << bitmap->height << ", pts=" << subtitle_frame.pts << std::endl;
                    }
                    
                    avsubtitle_free(&subtitle);
                    av_packet_unref(packet);
                    av_packet_free(&packet);
                    return true;
                } else {
                    // num_rects == 0 の場合は消去コマンド（字幕を消す）
                    DEBUG_LOG("get_next_subtitle_frame: 消去コマンドを検出しました (num_rects=0)");
                    
                    // PTS を設定
                    if (packet->pts != AV_NOPTS_VALUE) {
                        subtitle_frame.pts = packet->pts;
                    } else {
                        subtitle_frame.pts = subtitle.pts;
                    }
                    
                    // タイムスタンプを計算
                    AVStream* stream = format_ctx_->streams[subtitle_stream_index_];
                    double base_timestamp = 0.0;
                    if (subtitle_frame.pts != AV_NOPTS_VALUE) {
                        base_timestamp = pts_to_seconds(subtitle_frame.pts, stream->time_base);
                        subtitle_frame.timestamp = base_timestamp;
                    } else {
                        subtitle_frame.timestamp = 0.0;
                        base_timestamp = 0.0;
                    }
                    
                    // start_display_time と end_display_time を使用
                    if (subtitle.start_display_time != 0xFFFFFFFF && subtitle.end_display_time != 0xFFFFFFFF) {
                        subtitle_frame.start_time = base_timestamp + (subtitle.start_display_time / 1000.0);
                        subtitle_frame.end_time = base_timestamp + (subtitle.end_display_time / 1000.0);
                    } else {
                        subtitle_frame.start_time = base_timestamp;
                        subtitle_frame.end_time = base_timestamp;
                    }
                    
                    // bitmap は nullptr のまま（消去コマンドのため）
                    subtitle_frame.bitmap = nullptr;
                    subtitle_frame.x = 0;
                    subtitle_frame.y = 0;
                    
                    avsubtitle_free(&subtitle);
                    av_packet_unref(packet);
                    av_packet_free(&packet);
                    return true;
                }
            }
            
            avsubtitle_free(&subtitle);
        }
        av_packet_unref(packet);
    }
    
    av_packet_free(&packet);
    return false;
}

double FFmpegWrapper::pts_to_seconds(int64_t pts, const AVRational& time_base) const {
    if (pts == AV_NOPTS_VALUE) {
        return 0.0;
    }
    return pts * av_q2d(time_base);
}

void FFmpegWrapper::set_debug(bool debug) {
    debug_ = debug;
    // デバッグモードの場合はFFmpegのログレベルをINFOに、そうでない場合はFATALに設定
    if (debug) {
        av_log_set_level(AV_LOG_INFO);
    } else {
        av_log_set_level(AV_LOG_FATAL);
    }
}


void FFmpegWrapper::close() {
    if (codec_ctx_) {
        avcodec_free_context(&codec_ctx_);
        codec_ctx_ = nullptr;
    }
    
    if (format_ctx_) {
        avformat_close_input(&format_ctx_);
        format_ctx_ = nullptr;
    }
    
    subtitle_stream_index_ = -1;
}


