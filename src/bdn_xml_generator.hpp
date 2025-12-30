#ifndef BDN_XML_GENERATOR_HPP
#define BDN_XML_GENERATOR_HPP

#include <string>
#include <vector>
#include <optional>

struct SubtitleEvent {
    std::string in_tc;  // 開始タイムコード（HH:MM:SS:FF 形式）
    std::string out_tc; // 終了タイムコード（HH:MM:SS:FF 形式）
    std::string png_file; // PNG ファイル名
    int x = 0;  // X 座標
    int y = 0;  // Y 座標
    int width = 0;  // 幅
    int height = 0; // 高さ
};

struct BDNInfo {
    int video_width = 1920;
    int video_height = 1080;
    double fps = 29.97;
    std::string video_format = "1080p";  // VideoFormat (1080p, 1080i, 720p, 480p, 480i)
};

class BDNXmlGenerator {
public:
    BDNXmlGenerator(const BDNInfo& info);
    
    // 字幕イベントを追加
    void add_event(const SubtitleEvent& event);
    
    // BDN XML をファイルに書き込む
    bool write_to_file(const std::string& filename) const;
    
    // タイムコードを文字列に変換（秒から HH:MM:SS:FF 形式へ）
    static std::string time_to_tc(double seconds, double fps);
    
    // start_time を考慮してタイムコードを計算
    static double adjust_timestamp(double timestamp, double start_time);
    
    // VideoFormatを判定（canvas_heightとis_interlacedから）
    static std::string determine_video_format(int canvas_height, bool is_interlaced);

private:
    BDNInfo info_;
    std::vector<SubtitleEvent> events_;
    
    // XML エスケープ
    static std::string xml_escape(const std::string& str);
    
    // タイムコードのフォーマット（HH:MM:SS:FF）
    static std::string format_tc(int hours, int minutes, int seconds, int frames);
};

#endif // BDN_XML_GENERATOR_HPP

