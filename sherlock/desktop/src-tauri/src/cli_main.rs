use std::path::Path;
use std::sync::atomic::AtomicBool;

use sherlock_app_lib::config;
use sherlock_app_lib::models;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        print_usage();
        return;
    }

    let _ = cli_logger::init();

    let cmd = &args[1];
    let rest = &args[2..];

    match cmd.as_str() {
        "scan" => cmd_scan(rest),
        "status" => cmd_status(),
        "search" => cmd_search(rest),
        "roots" => cmd_roots(),
        "classify" => cmd_classify(rest),
        "add-root" => cmd_add_root(rest),
        "help" | "-h" | "--help" => print_usage(),
        _ => {
            eprintln!("sherlock: unknown command '{cmd}'");
            eprintln!("Run `sherlock help` for usage.");
        }
    }
}

fn print_usage() {
    println!("Usage: sherlock <command> [options]");
    println!();
    println!("Commands:");
    println!("  scan       <path>       Scan a directory (classify + thumbnail)");
    println!("  status                  Show database stats and active jobs");
    println!("  search     <query>      Search the catalog");
    println!("  roots                   List all root directories");
    println!("  classify   <file>       Classify a single file");
    println!("  add-root   <path>       Add a new root directory and scan it");
    println!("  help                    Show this help");
}

// ── Logger ────────────────────────────────────────────────────────────

mod cli_logger {
    use log::{Level, LevelFilter, Metadata, Record};

    pub(super) fn init() -> Result<(), log::SetLoggerError> {
        log::set_logger(&CLI_LOGGER)?;
        log::set_max_level(LevelFilter::Info);
        Ok(())
    }

    struct CliLogger;

    impl log::Log for CliLogger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            metadata.level() <= Level::Info
        }
        fn log(&self, record: &Record) {
            if !self.enabled(record.metadata()) {
                return;
            }
            match record.level() {
                Level::Error => eprintln!("error: {}", record.args()),
                Level::Warn => eprintln!("warning: {}", record.args()),
                Level::Info => println!("  {}", record.args()),
                _ => {}
            }
        }
        fn flush(&self) {}
    }

    static CLI_LOGGER: CliLogger = CliLogger;
}

// ── scan ──────────────────────────────────────────────────────────────

fn cmd_scan(args: &[String]) {
    if args.is_empty() {
        eprintln!("usage: sherlock scan <path> [--skip-classify]");
        return;
    }

    let raw_path = &args[0];
    let mut skip_classify = false;

    for arg in args[1..].iter() {
        match arg.as_str() {
            "--skip-classify" => skip_classify = true,
            _ => {
                eprintln!("sherlock scan: unknown option '{arg}'");
                return;
            }
        }
    }

    let canonical = match config::expand_and_canonicalize(raw_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("sherlock scan: invalid path '{raw_path}': {e}");
            return;
        }
    };

    let app_state = sherlock_app_lib::init_app();
    if app_state.read_only {
        eprintln!("sherlock scan: database is read-only");
        return;
    }

    let ctx = sherlock_app_lib::build_cli_scan_context(&app_state);

    let job = match sherlock_app_lib::db::create_or_resume_scan_job(&ctx.db_path, &canonical.display().to_string()) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("sherlock scan: failed to start scan job: {e}");
            return;
        }
    };

    let _ = sherlock_app_lib::db::adopt_child_files(&ctx.db_path, job.root_id, &job.root_path);

    println!("Scanning {}...", job.root_path);
    let cancel = AtomicBool::new(false);

    let summary =
        match sherlock_app_lib::scan::run_scan_job(&ctx, job.id, Some(&cancel), skip_classify) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("sherlock scan: scan failed: {e}");
                return;
            }
        };

    sherlock_app_lib::db::wal_checkpoint(&ctx.db_path).ok();
    let backup = app_state.paths.db_dir.join("index.sqlite.bak");
    sherlock_app_lib::db::backup_database(&ctx.db_path, &backup).ok();

    println!();
    println!("  Done. {} files scanned in {:.1}s", summary.scanned, summary.elapsed_ms as f64 / 1000.0);
    println!("  Added: {}, Modified: {}, Moved: {}", summary.added, summary.modified, summary.moved);
    println!("  Unchanged: {}, Deleted: {}", summary.unchanged, summary.deleted);
}

// ── status ────────────────────────────────────────────────────────────

fn cmd_status() {
    let app_state = sherlock_app_lib::init_app();

    let stats = match sherlock_app_lib::db::database_stats(&app_state.paths.db_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sherlock status: {e}");
            return;
        }
    };

    let db_size = pretty_size(stats.db_size_bytes);
    let thumbs_size = pretty_size(stats.thumbs_size_bytes);

    println!("Database");
    println!("  Roots:        {}", stats.roots);
    println!("  Files:        {}", stats.files);
    println!("  DB size:      {}", db_size);
    println!("  Thumbnails:   {}", thumbs_size);

    let jobs = sherlock_app_lib::db::list_resumable_scan_jobs(&app_state.paths.db_file)
        .unwrap_or_default();
    let active: Vec<_> = jobs.iter().filter(|j| j.status != "completed" && j.status != "failed").collect();

    if !active.is_empty() {
        println!();
        println!("Active Scan Jobs");
        for job in &active {
            println!("  Job #{}: {} — {} ({}%)", job.id, job.root_path, job.status, (job.progress_pct * 100.0) as u32);
        }
    }
}

// ── search ────────────────────────────────────────────────────────────

fn cmd_search(args: &[String]) {
    if args.is_empty() {
        eprintln!("usage: sherlock search <query> [--limit N]");
        return;
    }

    let mut query = String::new();
    let mut limit: u32 = 20;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--limit" {
            i += 1;
            if i < args.len() {
                limit = args[i].parse().unwrap_or(20);
            }
        } else if query.is_empty() {
            query = args[i].clone();
        } else {
            query.push(' ');
            query.push_str(&args[i]);
        }
        i += 1;
    }

    if query.is_empty() {
        eprintln!("usage: sherlock search <query>");
        return;
    }

    let app_state = sherlock_app_lib::init_app();

    let request = models::SearchRequest {
        query,
        limit: Some(limit),
        offset: Some(0),
        root_scope: Vec::new(),
        media_types: Vec::new(),
        min_confidence: None,
        date_from: None,
        date_to: None,
        sort_by: models::SortField::Relevance,
        sort_order: models::SortOrder::Desc,
    };

    let response = match sherlock_app_lib::db::search_images(&app_state.paths.db_file, &request) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sherlock search: {e}");
            return;
        }
    };

    if response.items.is_empty() {
        println!("  No results.");
        return;
    }

    println!();
    println!("  Results: {} (showing first {})", response.total, response.items.len());
    println!();
    for item in &response.items {
        let conf = (item.confidence * 100.0) as u32;
        let desc = truncate(&item.description, 60);
        println!("  [{:>3}%] {:>12}  {}", conf, item.media_type, item.rel_path);
        if !desc.is_empty() {
            println!("        {}", desc);
        }
    }
}

// ── roots ─────────────────────────────────────────────────────────────

fn cmd_roots() {
    let app_state = sherlock_app_lib::init_app();

    let roots = match sherlock_app_lib::db::list_roots(&app_state.paths.db_file) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sherlock roots: {e}");
            return;
        }
    };

    if roots.is_empty() {
        println!("  No roots configured.");
        return;
    }

    println!();
    println!("  ID  Files  Root Path");
    println!("  --  -----  ---------");
    for root in &roots {
        let path = truncate(&root.root_path, 70);
        println!("  {:>2}  {:>5}  {}", root.id, root.file_count, path);
    }
}

// ── classify ──────────────────────────────────────────────────────────

fn cmd_classify(args: &[String]) {
    if args.is_empty() {
        eprintln!("usage: sherlock classify <file>");
        return;
    }

    let file_path = Path::new(&args[0]);
    if !file_path.exists() {
        eprintln!("sherlock classify: file not found: {}", file_path.display());
        return;
    }

    let app_state = sherlock_app_lib::init_app();
    let ctx = sherlock_app_lib::build_cli_scan_context(&app_state);

    let is_video = sherlock_app_lib::video::is_video_file(file_path);
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let is_pdf = ext == "pdf";

    let result = if is_video {
        sherlock_app_lib::classify::classify_video(
            file_path,
            &ctx.provider,
            &ctx.tmp_dir,
            &ctx.surya_venv_dir,
            &ctx.surya_script,
        )
    } else if is_pdf {
        let passwords = sherlock_app_lib::db::get_all_pdf_password_strings(&ctx.db_path).unwrap_or_default();
        let pdfium = &ctx.pdfium_lib_path;
        let working_pw = sherlock_app_lib::pdf::try_passwords(file_path, pdfium, &passwords);
        if working_pw.is_none() && sherlock_app_lib::pdf::is_password_protected(file_path, pdfium) {
            println!("  File is password-protected and no matching password found.");
            return;
        }
        sherlock_app_lib::classify::classify_pdf(
            file_path,
            &ctx.provider,
            &ctx.tmp_dir,
            &ctx.surya_venv_dir,
            &ctx.surya_script,
            pdfium,
            working_pw.as_deref(),
        )
    } else {
        sherlock_app_lib::classify::classify_image(
            file_path,
            &ctx.provider,
            &ctx.tmp_dir,
            &ctx.surya_venv_dir,
            &ctx.surya_script,
        )
    };

    let conf = (result.confidence * 100.0) as u32;
    println!();
    println!("  {} — {}", file_path.display(), result.media_type);
    println!("  Confidence: {}%", conf);
    if !result.description.is_empty() {
        println!("  Description: {}", result.description);
    }
    if !result.extracted_text.is_empty() {
        let text = truncate(&result.extracted_text, 200);
        println!("  Text: {}", text);
    }
    if !result.canonical_mentions.is_empty() {
        println!("  Mentions: {}", result.canonical_mentions);
    }
}

// ── add-root ──────────────────────────────────────────────────────────

fn cmd_add_root(args: &[String]) {
    if args.is_empty() {
        eprintln!("usage: sherlock add-root <path>");
        return;
    }

    let root_path = &args[0];
    let canonical = match config::expand_and_canonicalize(root_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("sherlock add-root: invalid path '{root_path}': {e}");
            return;
        }
    };

    let app_state = sherlock_app_lib::init_app();
    if app_state.read_only {
        eprintln!("sherlock add-root: database is read-only");
        return;
    }

    match sherlock_app_lib::db::upsert_root(&app_state.paths.db_file, &canonical.display().to_string()) {
        Ok(root_id) => {
            println!("  Added root #{}: {}", root_id, canonical.display());
            // Start a scan for the new root
            let scan_args = vec![canonical.display().to_string()];
            cmd_scan(&scan_args);
        }
        Err(e) => {
            eprintln!("sherlock add-root: {e}");
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

fn pretty_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}
