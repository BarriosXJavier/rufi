
use crate::commands::{LaunchItem, ItemType};

pub fn fuzzy_search(query: &str, items: &[LaunchItem], max_results: usize) -> Vec<(LaunchItem, i32)> {
    let mut scored: Vec<(LaunchItem, i32)> = items
        .iter()
        .filter_map(|item: &LaunchItem| fuzzy_score(query, item).map(|score| (item.clone(), score)))
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.truncate(max_results);
    scored
}

fn fuzzy_score(query: &str, item: &LaunchItem) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }

    let query = query.to_lowercase();
    let name = item.display_name.to_lowercase();
    let command = item.command.to_lowercase();

    let type_bonus = match item.item_type {
        ItemType::Application => 50,
        ItemType::Command => 0,
    };

    if name == query || command == query {
        return Some(2000 + type_bonus);
    }

    if name.starts_with(&query) {
        return Some(1500 - query.len() as i32 + type_bonus);
    }

    if command.starts_with(&query) {
        return Some(1400 - query.len() as i32 + type_bonus);
    }

    if name.contains(&query) {
        return Some(1000 - query.len() as i32 + type_bonus);
    }

    if command.contains(&query) {
        return Some(900 - query.len() as i32 + type_bonus);
    }

    if let Some(desc) = &item.description {
        let desc = desc.to_lowercase();
        if desc.contains(&query) {
            return Some(600 - query.len() as i32 + type_bonus);
        }
    }

    let mut best_score: Option<i32> = None;

    for target in [&name, &command] {
        if let Some(score) = fuzzy_match_score(&query, target) {
            let adjusted_score = score + type_bonus;
            best_score = Some(best_score.map_or(adjusted_score, |s| s.max(adjusted_score)));
        }
    }

    best_score
}

fn fuzzy_match_score(query: &str, target: &str) -> Option<i32> {
    let mut query_chars = query.chars();
    let mut current_char = query_chars.next()?;
    let mut score = 200;
    let mut last_match = 0;
    let mut consecutive = 0;

    for (i, target_char) in target.chars().enumerate() {
        if target_char == current_char {
            let gap = i - last_match;
            if gap == 1 {
                consecutive += 1;
                score += consecutive * 10; // Bonus for consecutive matches
            } else {
                consecutive = 0;
                score -= gap as i32; // Penalize gaps
            }

            last_match = i;
            if let Some(next) = query_chars.next() {
                current_char = next;
            } else {
                return Some(score);
            }
        }
    }

    None
}
