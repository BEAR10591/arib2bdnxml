#include "bdn_xml_generator.hpp"
#include <iostream>
#include <fstream>
#include <iomanip>
#include <sstream>
#include <cmath>

BDNXmlGenerator::BDNXmlGenerator(const BDNInfo& info) : info_(info) {
}

void BDNXmlGenerator::add_event(const SubtitleEvent& event) {
    events_.push_back(event);
}

bool BDNXmlGenerator::write_to_file(const std::string& filename) const {
    std::ofstream file(filename);
    if (!file.is_open()) {
        std::cerr << "エラー: ファイルを開けませんでした: " << filename << std::endl;
        return false;
    }
    
    // XML ヘッダー
    file << "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";
    file << "<BDN Version=\"0.93\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" "
         << "xsi:noNamespaceSchemaLocation=\"BDN.xsd\">\n";
    
    // Description
    file << "  <Description>\n";
    file << "    <Name Title=\"BDN Subtitle\"/>\n";
    file << "    <Language Code=\"und\"/>\n";
    file << "    <Format VideoFormat=\"" << info_.video_format << "\" ";
    file << "FrameRate=\"" << std::fixed << std::setprecision(3) << info_.fps << "\" ";
    file << "DropFrame=\"False\"/>\n";
    file << "  </Description>\n";
    
    // Events
    file << "  <Events>\n";
    for (const auto& event : events_) {
        file << "    <Event InTC=\"" << xml_escape(event.in_tc) << "\" "
             << "OutTC=\"" << xml_escape(event.out_tc) << "\" "
             << "Forced=\"False\">\n";
        file << "      <Graphic Width=\"" << event.width << "\" "
             << "Height=\"" << event.height << "\" "
             << "X=\"" << event.x << "\" "
             << "Y=\"" << event.y << "\">"
             << xml_escape(event.png_file) << "</Graphic>\n";
        file << "    </Event>\n";
    }
    file << "  </Events>\n";
    
    // フッター
    file << "</BDN>\n";
    
    return true;
}

std::string BDNXmlGenerator::time_to_tc(double seconds, double fps) {
    if (seconds < 0) {
        seconds = 0;
    }
    
    // 総フレーム数を計算（正確なFPS値を使用）
    int total_frames = static_cast<int>(std::round(seconds * fps));
    
    // BDN XMLのタイムコード形式では、フレーム番号は整数で、0からFPS-1の範囲
    // 29.97fpsの場合、実際には30フレーム/秒として扱われる
    // 累積誤差を避けるため、総フレーム数から直接時間、分、秒、フレームを計算
    int fps_int = static_cast<int>(std::round(fps));
    int frames_per_hour = fps_int * 3600;
    int frames_per_minute = fps_int * 60;
    
    int hours = total_frames / frames_per_hour;
    int remaining_frames = total_frames % frames_per_hour;
    int minutes = remaining_frames / frames_per_minute;
    remaining_frames = remaining_frames % frames_per_minute;
    int secs = remaining_frames / fps_int;
    int frames = remaining_frames % fps_int;
    
    // 秒が60以上になる場合は分に繰り上げ
    if (secs >= 60) {
        minutes += secs / 60;
        secs = secs % 60;
    }
    // 分が60以上になる場合は時間に繰り上げ
    if (minutes >= 60) {
        hours += minutes / 60;
        minutes = minutes % 60;
    }
    
    return format_tc(hours, minutes, secs, frames);
}

double BDNXmlGenerator::adjust_timestamp(double timestamp, double start_time) {
    // start_time を 00:00:00.000 として扱うため、timestamp から start_time を減算
    return timestamp - start_time;
}

std::string BDNXmlGenerator::determine_video_format(int canvas_height, bool is_interlaced) {
    // canvas_heightとis_interlacedからVideoFormatを判定
    if (canvas_height == 1080) {
        return is_interlaced ? "1080i" : "1080p";
    } else if (canvas_height == 720) {
        // 720iはBDMV仕様上存在しないため、常に720p
        return "720p";
    } else if (canvas_height == 480) {
        return is_interlaced ? "480i" : "480p";
    } else {
        // その他の解像度の場合はデフォルトで1080pを返す
        // エラーを出すべきか、デフォルト値を返すべきかは要検討
        return "1080p";
    }
}

std::string BDNXmlGenerator::format_tc(int hours, int minutes, int seconds, int frames) {
    std::ostringstream oss;
    oss << std::setfill('0') << std::setw(2) << hours << ":"
        << std::setfill('0') << std::setw(2) << minutes << ":"
        << std::setfill('0') << std::setw(2) << seconds << ":"
        << std::setfill('0') << std::setw(2) << frames;
    return oss.str();
}

std::string BDNXmlGenerator::xml_escape(const std::string& str) {
    std::string result;
    result.reserve(str.length());
    
    for (char c : str) {
        switch (c) {
            case '&':  result += "&amp;";  break;
            case '<':  result += "&lt;";   break;
            case '>':  result += "&gt;";   break;
            case '"':  result += "&quot;"; break;
            case '\'': result += "&apos;"; break;
            default:   result += c;        break;
        }
    }
    
    return result;
}

