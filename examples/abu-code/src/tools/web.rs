use std::time::Duration;
use regex::Regex;

// ============================================================================
// Helpers
// ============================================================================

/// Strip HTML tags and convert to readable text, truncated to `max_chars`.
fn html_to_text(html: &str, max_chars: usize) -> String {
    // Remove script and style blocks
    let re_script = Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
    let re_style = Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    let re_block = Regex::new(r"(?i)</?(?:div|p|h[1-6]|li|tr|br|hr|article|section|header|footer|nav|main|aside|table|ul|ol|dl|dt|dd|blockquote|pre|figure|figcaption|form|fieldset)[^>]*/?>").unwrap();
    let re_tag = Regex::new(r"<[^>]+>").unwrap();
    let html = re_script.replace_all(html, "");
    let html = re_style.replace_all(&html, "");
    let html = re_block.replace_all(&html, "\n");
    let html = re_tag.replace_all(&html, "");

    // Decode common HTML entities
    let html = html
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&rsquo;", "'")
        .replace("&lsquo;", "'")
        .replace("&rdquo;", "\"")
        .replace("&ldquo;", "\"");

    // Collapse multiple blank lines
    let re_blanks = Regex::new(r"\n{3,}").unwrap();
    let mut text = re_blanks.replace_all(&html, "\n\n").to_string();

    // Trim and truncate
    text = text.trim().to_string();
    if text.len() > max_chars {
        text.truncate(max_chars);
        text.push_str("...");
    }
    text
}

/// Parse DuckDuckGo Lite search results HTML into formatted text.
fn parse_ddg_results(html: &str) -> String {
    // DDG Lite returns results in <a> tags with class="result-link" and snippets in
    // <td class="result-snippet">. We do a lightweight parse.
    let mut results: Vec<String> = Vec::new();
    let mut count = 0u32;

    // Each result row: <a rel="nofollow" class="result-link" href="URL">Title</a>
    // followed by <span class="result-snippet">Snippet</span>
    let re_result =
        Regex::new(r#"<a[^>]*class="result-link"[^>]*href="([^"]*)"[^>]*>([^<]*)</a>"#).unwrap();
    let re_snippet = Regex::new(r#"<span[^>]*class="result-snippet"[^>]*>(.*?)</span>"#).unwrap();

    let mut snippets: Vec<String> = Vec::new();
    for cap in re_snippet.captures_iter(html) {
        let snippet = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let snippet = html_to_text(snippet, 300);
        snippets.push(snippet);
    }

    let mut snippet_idx = 0usize;
    for cap in re_result.captures_iter(html) {
        if count >= 10 {
            break;
        }
        let url = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let title = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let title = html_to_text(title, 120);

        let snippet = snippets.get(snippet_idx).cloned().unwrap_or_default();
        snippet_idx += 1;

        results.push(format!("{}. {}\n   {}\n   {}", count + 1, title, url, snippet));
        count += 1;
    }

    if results.is_empty() {
        "No results found. The search engine may have blocked the request — try again later or check your network.".to_string()
    } else {
        results.join("\n\n")
    }
}

/// Percent-encode a query string for a URL.
fn url_encode(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push('%');
                encoded.push(hex_char(byte >> 4));
                encoded.push(hex_char(byte & 0x0F));
            }
        }
    }
    encoded
}

fn hex_char(b: u8) -> char {
    match b {
        0..=9 => (b'0' + b) as char,
        _ => (b'A' + (b - 10)) as char,
    }
}

// ============================================================================
// WebFetch — fetch a URL and return readable text
// ============================================================================

#[abu_tool::tool(
    struct_name = WebFetch,
    name = "web_fetch",
    description = "Fetch content from a URL and return it as readable text (HTML stripped). Use for reading documentation, API references, or any web page."
)]
pub async fn web_fetch(url: String) -> String {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (compatible; abu-code/0.1; +https://github.com/MoleSir/abu)")
        .build()
    {
        Ok(c) => c,
        Err(e) => return format!("Error building HTTP client: {}", e),
    };

    match client.get(&url).send().await {
        Ok(resp) => {
            let status = resp.status();
            let content_type = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();

            match resp.text().await {
                Ok(body) => {
                    if body.is_empty() {
                        return format!("Empty response from {}", url);
                    }
                    if content_type.contains("text/html") || content_type.contains("text/plain") {
                        html_to_text(&body, 10000)
                    } else if content_type.contains("application/json") {
                        // Truncate JSON to a reasonable size
                        if body.len() > 10000 {
                            let mut truncated = body[..10000].to_string();
                            truncated.push_str("...");
                            truncated
                        } else {
                            body
                        }
                    } else {
                        format!(
                            "{} {} — {} bytes of {} content (showing text preview):\n{}",
                            status.as_u16(),
                            status.canonical_reason().unwrap_or(""),
                            body.len(),
                            content_type,
                            html_to_text(&body, 5000)
                        )
                    }
                }
                Err(e) => format!("Error reading response body: {}", e),
            }
        }
        Err(e) => format!("Error fetching URL '{}': {}", url, e),
    }
}

// ============================================================================
// WebSearch — search the web via DuckDuckGo
// ============================================================================

#[abu_tool::tool(
    struct_name = WebSearch,
    name = "web_search",
    description = "Search the web and return results with titles, URLs, and snippets. Use for finding documentation, solutions to errors, or current information."
)]
pub async fn web_search(query: String) -> String {
    let url = format!(
        "https://lite.duckduckgo.com/lite/?q={}",
        url_encode(&query)
    );

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (compatible; abu-code/0.1; +https://github.com/MoleSir/abu)")
        .build()
    {
        Ok(c) => c,
        Err(e) => return format!("Error building HTTP client: {}", e),
    };

    match client.get(&url).send().await {
        Ok(resp) => match resp.text().await {
            Ok(html) => parse_ddg_results(&html),
            Err(e) => format!("Error reading search results: {}", e),
        },
        Err(e) => format!(
            "Error searching for '{}': {}. Check your network connection.",
            query, e
        ),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_encode_basic() {
        assert_eq!(url_encode("hello world"), "hello+world");
        assert_eq!(url_encode("rust & go"), "rust+%26+go");
        assert_eq!(url_encode("a/b?c=d"), "a%2Fb%3Fc%3Dd");
        assert_eq!(url_encode("keep-_.~"), "keep-_.~");
    }

    #[test]
    fn test_url_encode_chinese() {
        let encoded = url_encode("你好");
        assert!(encoded.starts_with("%E4%BD%A0%E5%A5%BD") || encoded.contains("%"));
    }

    #[test]
    fn test_html_to_text_strips_tags() {
        let html = "<html><body><p>Hello World</p></body></html>";
        let text = html_to_text(html, 500);
        assert!(text.contains("Hello World"));
        assert!(!text.contains("<p>"));
        assert!(!text.contains("<html>"));
    }

    #[test]
    fn test_html_to_text_decodes_entities() {
        let html = "&lt;hello&gt; &amp; &quot;world&quot;";
        let text = html_to_text(html, 500);
        assert!(text.contains("<hello>"));
        assert!(text.contains('&'));
        assert!(text.contains("\"world\""));
    }

    #[test]
    fn test_html_to_text_truncates() {
        let html = "<p>abcdefghijklmnop</p>";
        let text = html_to_text(html, 5);
        assert!(text.ends_with("..."));
        assert!(text.len() <= 8);
    }

    #[test]
    fn test_html_to_text_removes_script() {
        let html = "<html><script>alert('xss')</script><p>safe</p></html>";
        let text = html_to_text(html, 500);
        assert!(text.contains("safe"));
        assert!(!text.contains("alert"));
        assert!(!text.contains("script"));
    }

    #[test]
    fn test_parse_ddg_results_empty() {
        let result = parse_ddg_results("<html></html>");
        assert!(result.contains("No results found"));
    }
}
