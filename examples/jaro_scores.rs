//! Print Jaro-Winkler similarity scores for sample pairs.

use simple_agents_healing::string_utils::jaro_winkler;

fn main() {
    let pairs = [
        ("is_verfied", "isVerfied"),
        ("IS_VERFIED", "isVerfied"),
    ];

    for (a, b) in pairs {
        let score = jaro_winkler(a, b);
        println!("{} <-> {} = {:.4}", a, b, score);
    }
}
