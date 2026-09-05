use std::io::{self, Write};
use std::process::Command;
use std::thread;
use std::time::Duration;

const HIST: usize = 30;
const CAT_W: usize = 22;

// we love catppuccin frappe
const GREEN: (u8, u8, u8) = (166, 209, 137);
const YELLOW: (u8, u8, u8) = (229, 200, 144);
const RED: (u8, u8, u8) = (231, 130, 132);
const BLUE: (u8, u8, u8) = (140, 170, 238);
const TEAL: (u8, u8, u8) = (129, 200, 190);
const PEACH: (u8, u8, u8) = (239, 159, 118);
const SKY: (u8, u8, u8) = (153, 209, 219);
const TEXT: (u8, u8, u8) = (198, 208, 245);
const OVERLAY1: (u8, u8, u8) = (131, 139, 167);
const OVERLAY0: (u8, u8, u8) = (115, 121, 148);
const SURFACE2: (u8, u8, u8) = (98, 104, 128);
const SURFACE0: (u8, u8, u8) = (65, 69, 89);

struct Gpu {
    index: String,
    name: String,
    load: f64,
    mem_used_mib: f64,
    mem_total_mib: f64,
    temp_c: f64,
    power_w: Option<f64>,
    power_limit_w: Option<f64>,
}

fn main() {
    let mut per_gpu: Vec<Vec<f64>> = Vec::new();
    let mut tick = 0u64;

    loop {
        let gpus = gpus().unwrap_or_default();
        if gpus.len() != per_gpu.len() {
            per_gpu = vec![Vec::new(); gpus.len()];
        }
        for (h, g) in per_gpu.iter_mut().zip(&gpus) {
            push(h, g.load);
        }

        let mut out = String::from("\x1b[H\x1b[2J");
        out.push_str(&frame(&gpus, &per_gpu, tick));
        out.push_str("\x1b[J");
        print!("{out}");
        io::stdout().flush().ok();

        tick += 1;
        thread::sleep(Duration::from_secs(1));
    }
}

fn push(v: &mut Vec<f64>, x: f64) {
    // keep the history from getting out of hand
    v.push(x);
    if v.len() > HIST {
        v.remove(0);
    }
}

fn sgr(code: &str, s: &str) -> String {
    format!("\x1b[{code}m{s}\x1b[0m")
}

fn rgb(r: u8, g: u8, b: u8, s: &str) -> String {
    format!("\x1b[38;2;{r};{g};{b}m{s}\x1b[0m")
}

fn col(c: (u8, u8, u8), s: &str) -> String {
    rgb(c.0, c.1, c.2, s)
}

fn bold_col(c: (u8, u8, u8), s: &str) -> String {
    format!("\x1b[1;38;2;{};{};{}m{s}\x1b[0m", c.0, c.1, c.2)
}

fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
    (f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
}

fn tone(load: f64) -> (u8, u8, u8) {
    // color that reflects how hard the gpu is working
    let t = (load / 100.0).clamp(0.0, 1.0);
    let (g, y, r) = (GREEN, YELLOW, RED);
    if t < 0.5 {
        lerp(g, y, t * 2.0)
    } else {
        lerp(y, r, (t - 0.5) * 2.0)
    }
}

fn heat(t: f64) -> (u8, u8, u8) {
    let stops = [
        (BLUE, 0.0),
        (TEAL, 0.28),
        (YELLOW, 0.52),
        (PEACH, 0.76),
        (RED, 1.0),
    ];
    for w in stops.windows(2) {
        let ((c0, t0), (c1, t1)) = (w[0], w[1]);
        if t <= t1 {
            return lerp(c0, c1, (t - t0) / (t1 - t0));
        }
    }
    stops[4].0
}

fn vis_len(s: &str) -> usize {
    let mut n = 0;
    let mut esc = false;
    for c in s.chars() {
        if esc {
            if c == 'm' {
                esc = false;
            }
        } else if c == '\x1b' {
            esc = true;
        } else {
            n += 1;
        }
    }
    n
}

fn pad(s: &str, w: usize) -> String {
    let mut out = s.to_string();
    while vis_len(&out) < w {
        out.push(' ');
    }
    out
}

fn bar(frac: f64, width: usize, rainbow: bool) -> String {
    let n = ((frac.clamp(0.0, 1.0) * width as f64) as usize).min(width);
    let mut out = String::new();
    for i in 0..width {
        if i < n {
            let c = if rainbow {
                heat(i as f64 / width.max(1) as f64)
            } else {
                tone(frac * 100.0)
            };
            out.push_str(&col(c, "█"));
        } else {
            out.push_str(&col(SURFACE0, "░"));
        }
    }
    out
}

fn brick(v: f64) -> char {
    const B: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    B[(((v / 100.0) * 8.0) as usize).min(7)]
}

fn spark(hist: &[f64]) -> String {
    let mut out = String::new();
    for v in hist {
        out.push_str(&col(heat(v / 100.0), &brick(*v).to_string()));
    }
    out
}

fn mood(load: f64) -> (&'static str, &'static str) {
    match load as i64 {
        0 => ("- -", "w"),
        1..=24 => ("o o", "v"),
        25..=49 => ("^ ^", "w"),
        50..=74 => ("O O", "o"),
        75..=99 => ("@ @", "~"),
        _ => ("* *", "~"),
    }
}

fn cat_lines(load: f64, closed: bool) -> Vec<String> {
    // how the cat looks right now
    let t = tone(load);
    let (eyes, mouth) = mood(load);
    let eyes = if closed { "- -" } else { eyes };
    let mut out = Vec::new();
    out.push(col(t, "     /\\_/\\"));
    out.push(col(t, &format!("    ( {eyes} )")));
    out.push(col(t, &format!("     > {mouth} <")));
    out.push(String::new());
    let pct = format!("{load:.0}%");
    let lead = 7 - pct.len() as i64 / 2;
    out.push(bold_col(
        t,
        &format!("{}{pct}", " ".repeat(lead.max(0) as usize)),
    ));
    out
}

fn gpus() -> Result<Vec<Gpu>, String> {
    let out = Command::new("nvidia-smi")
        .args([
            "--query-gpu=index,name,utilization.gpu,memory.used,memory.total,temperature.gpu,\
power.draw,power.limit",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .map_err(|e| format!("`nvidia-smi` failed: {e}"))?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }

    let mut list = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let f: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if f.len() < 8 {
            continue;
        }
        let num = |s: &str| s.parse::<f64>().ok();
        let opt = |s: &str| {
            let s = s.trim();
            if s.is_empty() || s.eq_ignore_ascii_case("N/A") || s.starts_with('[') {
                None
            } else {
                s.parse::<f64>().ok()
            }
        };
        list.push(Gpu {
            index: f[0].to_string(),
            name: f[1].to_string(),
            load: num(f[2]).unwrap_or(0.0),
            mem_used_mib: num(f[3]).unwrap_or(0.0),
            mem_total_mib: num(f[4]).unwrap_or(0.0),
            temp_c: num(f[5]).unwrap_or(0.0),
            power_w: opt(f[6]),
            power_limit_w: opt(f[7]),
        });
    }
    Ok(list)
}

fn frame(gpus: &[Gpu], per_gpu: &[Vec<f64>], tick: u64) -> String {
    let avg = if gpus.is_empty() {
        0.0
    } else {
        gpus.iter().map(|g| g.load).sum::<f64>() / gpus.len() as f64
    };
    let tc = tone(avg);
    let closed = tick % 3 == 0;
    let left = cat_lines(avg, closed);

    let mut right: Vec<String> = Vec::new();
    if gpus.is_empty() {
        right.push(bold_col(SKY, "   /\\_/\\  no GPU signal"));
        right.push(col(OVERLAY0, "   waiting for a GPU…"));
    }
    for (i, gp) in gpus.iter().enumerate() {
        if i > 0 {
            right.push(String::new());
        }
        let gc = tone(gp.load);
        right.push(bold_col(TEXT, &format!("  GPU {}  {}", gp.index, gp.name)));
        right.push(format!(
            "   load  {} {}",
            bar(gp.load / 100.0, 24, false),
            bold_col(gc, &format!("{:>3.0}%", gp.load))
        ));
        let mem = gp.mem_total_mib.max(0.001);
        let mf = (gp.mem_used_mib / mem).clamp(0.0, 1.0);
        right.push(format!(
            "   mem   {} {} / {} GiB",
            bar(mf, 24, false),
            col(gc, &format!("{:>5.1}", gp.mem_used_mib / 1024.0)),
            format!("{:.1}", mem / 1024.0)
        ));

        match (gp.power_w, gp.power_limit_w) {
            (Some(w), Some(lim)) if lim > 0.0 => {
                let frac = (w / lim).clamp(0.0, 1.0);
                let pc = tone(frac * 100.0);
                right.push(format!(
                    "   power {} {} / {} W",
                    bar(frac, 24, false),
                    bold_col(pc, &format!("{:>4.0}", w)),
                    format!("{:.0}", lim)
                ));
            }
            (Some(w), _) => {
                right.push(format!(
                    "   power {} {} W",
                    col(SURFACE0, &"░".repeat(24)),
                    bold_col(tone(50.0), &format!("{:>4.0}", w))
                ));
            }
            _ => {
                right.push(format!(
                    "   power {} {} W",
                    col(SURFACE0, &"░".repeat(24)),
                    col(OVERLAY0, "  n/a")
                ));
            }
        }

        right.push(format!(
            "   temp  {}",
            col(heat(gp.temp_c / 100.0), &format!("{:>3.0}°C", gp.temp_c))
        ));

        if let Some(h) = per_gpu.get(i) {
            right.push(format!("   hist  {}", spark(h)));
        }
    }

    let mut out = String::new();
    out.push_str(&col(SURFACE2, &format!("┌{}┐\n", "─".repeat(46))));
    let left_t = format!(
        "{}{}",
        bold_col(tc, "gpucat"),
        col(tc, &format!("   {avg:.0}%"))
    );
    let right_t = col(OVERLAY1, "· all GPUs");
    let gap = 46 - vis_len(&left_t) - vis_len(&right_t);
    out.push_str(&col(SURFACE2, "│"));
    out.push_str(&left_t);
    for _ in 0..gap.max(0) {
        out.push(' ');
    }
    out.push_str(&right_t);
    out.push_str(&col(SURFACE2, "│\n"));
    out.push_str(&col(SURFACE2, &format!("└{}┘\n\n", "─".repeat(46))));

    let h = left.len().max(right.len());
    for i in 0..h {
        let l = left.get(i).map(|s| s.as_str()).unwrap_or("");
        let r = right.get(i).map(|s| s.as_str()).unwrap_or("");
        out.push_str(&pad(l, CAT_W));
        out.push(' ');
        out.push_str(r);
        out.push('\n');
    }
    out
}
