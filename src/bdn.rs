use std::fs::File;
use std::io::{BufWriter, Write};

/// BDN metadata (frame rate, format). Written to BDN XML Description/Format.
#[derive(Debug, Clone)]
pub struct BdnInfo {
    pub fps: f64,
    pub video_format: String,
}

/// A single subtitle event (one graphic with InTC/OutTC and PNG reference).
#[derive(Debug, Clone)]
pub struct SubtitleEvent {
    pub in_tc: String,
    pub out_tc: String,
    pub png_file: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Converts seconds to BDN timecode HH:MM:SS:FF (frame index 0..fps_int-1).
pub fn time_to_tc(seconds: f64, fps: f64) -> String {
    let seconds = if seconds < 0.0 { 0.0 } else { seconds };
    let total_frames = (seconds * fps).round() as i32;
    let fps_int = fps.round() as i32;
    let frames_per_hour = fps_int * 3600;
    let frames_per_minute = fps_int * 60;

    let mut hours = total_frames / frames_per_hour;
    let mut remaining = total_frames % frames_per_hour;
    let mut minutes = remaining / frames_per_minute;
    remaining %= frames_per_minute;
    let mut secs = remaining / fps_int;
    let frames = remaining % fps_int;

    if secs >= 60 {
        minutes += secs / 60;
        secs %= 60;
    }
    if minutes >= 60 {
        hours += minutes / 60;
        minutes %= 60;
    }

    format_tc(hours, minutes, secs, frames)
}

/// Adjusts timestamp so that start_time is treated as 00:00:00.000.
pub fn adjust_timestamp(timestamp: f64, start_time: f64) -> f64 {
    timestamp - start_time
}

fn format_tc(hours: i32, minutes: i32, seconds: i32, frames: i32) -> String {
    format!(
        "{:02}:{:02}:{:02}:{:02}",
        hours, minutes, seconds, frames
    )
}

/// Format FPS for BDN XML. Output "29.97" for 29.970, "24" for 24.000; other rates keep 3 decimals.
fn format_fps(fps: f64) -> String {
    let s = format!("{:.3}", fps);
    match s.as_str() {
        "29.970" => "29.97".to_string(),
        "24.000" => "24".to_string(),
        _ => s,
    }
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// BDN XML format conforms to [BDSup2Sub Supported Formats](https://github.com/mjuhasz/BDSup2Sub/wiki/Supported-Formats#sony-bdn-xml-format).
/// Writes BDN 0.93 XML to a file.
pub struct BdnXmlGenerator {
    info: BdnInfo,
    events: Vec<SubtitleEvent>,
}

impl BdnXmlGenerator {
    pub fn new(info: BdnInfo) -> Self {
        BdnXmlGenerator {
            info,
            events: Vec::new(),
        }
    }

    pub fn add_event(&mut self, event: &SubtitleEvent) {
        self.events.push(event.clone());
    }

    pub fn write_to_file(&self, path: &str) -> anyhow::Result<()> {
        let f = File::create(path).map_err(|e| anyhow::anyhow!("Failed to open file: {}: {}", path, e))?;
        let mut w = BufWriter::new(f);

        writeln!(w, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>")?;
        writeln!(
            w,
            "<BDN Version=\"0.93\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"BD-03-006-0093b BDN File Format.xsd\">"
        )?;
        writeln!(w, "  <Description>")?;
        writeln!(w, "    <Name Title=\"BDN Subtitle\" Content=\"\"/>")?;
        writeln!(w, "    <Language Code=\"und\"/>")?;
        writeln!(
            w,
            "    <Format VideoFormat=\"{}\" FrameRate=\"{}\" DropFrame=\"False\"/>",
            self.info.video_format,
            format_fps(self.info.fps)
        )?;
        let (first_tc, last_tc) = if let (Some(first), Some(last)) = (self.events.first(), self.events.last()) {
            (first.in_tc.as_str(), last.out_tc.as_str())
        } else {
            ("00:00:00:00", "00:00:00:00")
        };
        writeln!(
            w,
            "    <Events Type=\"Graphic\" FirstEventInTC=\"{}\" LastEventOutTC=\"{}\" NumberofEvents=\"{}\"/>",
            xml_escape(first_tc),
            xml_escape(last_tc),
            self.events.len()
        )?;
        writeln!(w, "  </Description>")?;
        writeln!(w, "  <Events>")?;

        for event in &self.events {
            writeln!(
                w,
                "    <Event InTC=\"{}\" OutTC=\"{}\" Forced=\"False\">",
                xml_escape(&event.in_tc),
                xml_escape(&event.out_tc)
            )?;
            writeln!(
                w,
                "      <Graphic Width=\"{}\" Height=\"{}\" X=\"{}\" Y=\"{}\">{}</Graphic>",
                event.width,
                event.height,
                event.x,
                event.y,
                xml_escape(&event.png_file)
            )?;
            writeln!(w, "    </Event>")?;
        }

        writeln!(w, "  </Events>")?;
        writeln!(w, "</BDN>")?;
        w.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_to_tc() {
        assert_eq!(time_to_tc(0.0, 29.97), "00:00:00:00");
        assert_eq!(time_to_tc(1.0, 30.0), "00:00:01:00");
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("a<b"), "a&lt;b");
        assert_eq!(xml_escape("a&amp;b"), "a&amp;amp;b");
    }
}
