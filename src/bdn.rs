use std::fs::File;
use std::io::{BufWriter, Write};

/// BDN metadata (video dimensions, frame rate, format).
#[derive(Debug, Clone)]
#[allow(dead_code)] // video_width/video_height kept for BDN spec / future use
pub struct BdnInfo {
    pub video_width: i32,
    pub video_height: i32,
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

/// Determines VideoFormat string from canvas height and interlaced flag.
pub fn determine_video_format(canvas_height: i32, is_interlaced: bool) -> &'static str {
    match canvas_height {
        1080 => if is_interlaced { "1080i" } else { "1080p" },
        720 => "720p",
        480 => if is_interlaced { "480i" } else { "480p" },
        _ => "1080p",
    }
}

fn format_tc(hours: i32, minutes: i32, seconds: i32, frames: i32) -> String {
    format!(
        "{:02}:{:02}:{:02}:{:02}",
        hours, minutes, seconds, frames
    )
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
            "<BDN Version=\"0.93\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"BDN.xsd\">"
        )?;
        writeln!(w, "  <Description>")?;
        writeln!(w, "    <Name Title=\"BDN Subtitle\"/>")?;
        writeln!(w, "    <Language Code=\"und\"/>")?;
        writeln!(
            w,
            "    <Format VideoFormat=\"{}\" FrameRate=\"{:.3}\" DropFrame=\"False\"/>",
            self.info.video_format, self.info.fps
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
    fn test_determine_video_format() {
        assert_eq!(determine_video_format(1080, false), "1080p");
        assert_eq!(determine_video_format(1080, true), "1080i");
        assert_eq!(determine_video_format(720, true), "720p");
        assert_eq!(determine_video_format(480, true), "480i");
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("a<b"), "a&lt;b");
        assert_eq!(xml_escape("a&amp;b"), "a&amp;amp;b");
    }
}
