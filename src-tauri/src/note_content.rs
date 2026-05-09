pub fn parse_id_from_content(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut in_front_matter = false;

    for line in &lines {
        if line.trim() == "---" {
            if !in_front_matter {
                in_front_matter = true;
            } else {
                break;
            }
        } else if in_front_matter && line.starts_with("id:") {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() == 2 {
                return Some(parts[1].trim().to_string());
            }
        }
    }

    None
}

pub fn extract_content_only(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut content_start = 0;

    while content_start < lines.len() {
        if let Some(start_idx) = lines[content_start..].iter().position(|line| line.trim() == "---") {
            let actual_start_idx = content_start + start_idx;

            if let Some(end_idx) = lines[actual_start_idx + 1..].iter().position(|line| line.trim() == "---") {
                let actual_end_idx = actual_start_idx + 1 + end_idx;
                let mut found_id = false;
                let mut found_created_at = false;

                for line in lines.iter().take(actual_end_idx).skip(actual_start_idx + 1) {
                    let line = line.trim();
                    if line.starts_with("id:") {
                        found_id = true;
                    } else if line.starts_with("createdAt:") {
                        found_created_at = true;
                    }
                }

                if found_id && found_created_at {
                    content_start = if actual_end_idx + 1 < lines.len() && lines[actual_end_idx + 1].is_empty() {
                        actual_end_idx + 2
                    } else {
                        actual_end_idx + 1
                    };
                    continue;
                }
            }
        }

        break;
    }

    if content_start < lines.len() {
        lines[content_start..].join("\n")
    } else {
        String::new()
    }
}

pub fn build_full_content(id: &str, created_at: &str, content: &str) -> String {
    format!("---\nid: {}\ncreatedAt: {}\n---\n{}", id, created_at, content)
}

pub fn extract_first_line_preview(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.chars().take(50).collect());
        }
    }

    None
}

pub fn extract_created_at_from_content(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut in_front_matter = false;

    for line in &lines {
        if line.trim() == "---" {
            if !in_front_matter {
                in_front_matter = true;
            } else {
                break;
            }
        } else if in_front_matter && line.starts_with("createdAt:") {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() == 2 {
                return Some(parts[1].trim().to_string());
            }
        }
    }

    None
}
