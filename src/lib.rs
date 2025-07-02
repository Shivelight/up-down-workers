use serde::{Deserialize, Serialize};
use worker::*;

#[derive(Deserialize)]
struct InputUrl {
    url: String,
}

#[derive(Serialize, Clone)]
struct ProbeResult {
    #[serde(rename = "type")]
    probe_type: String,
    url: String,
    status: String,
    status_code: Option<u16>,
    status_text: String,
    duration_ms: Option<u64>,
}

#[derive(Serialize, Clone)]
struct FinalResponse {
    requested_url: String,
    results: Vec<ProbeResult>,
}

#[event(fetch)]
async fn fetch(mut req: Request, env: Env, ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let secret_api_key = env.secret("API_KEY")?.to_string();
    let req_api_key = req.headers().get("x-api-key")?;

    if req_api_key.is_none() || req_api_key.unwrap() != secret_api_key {
        return Response::error("Unauthorized", 401);
    };

    let target_url = match req.method() {
        Method::Post => req.json::<InputUrl>().await,
        Method::Get => req.query::<InputUrl>(),
        _ => return Err(Error::from("Method not allowed. Use GET or POST.")),
    };

    let mut target_url = match target_url {
        Ok(input_url) => input_url.url,
        Err(e) => return Response::error(e.to_string(), 400),
    };

    if !target_url.starts_with("http") {
        target_url = format!("https://{target_url}");
    }

    let target_url = Url::parse(&target_url)?;
    let cache_key = target_url.to_string();

    let cache = Cache::default();
    if let Ok(Some(cached)) = cache.get(&cache_key, false).await {
        let new_headers = cached.headers().clone();
        new_headers.set("X-Worker-Cache", "HIT")?;
        return Ok(cached.with_headers(new_headers));
    }

    let mut unique_target = std::collections::HashSet::new();
    let mut probes: Vec<(String, String)> = Vec::new();

    let Some(host) = target_url.host_str() else {
        return Err(Error::from("Host is missing."));
    };

    let host_url = format!("{}://{}", target_url.scheme(), host);
    if unique_target.insert(&host_url) {
        probes.push((host_url.to_string(), "host".to_string()));
    }

    if let Some(domain) = psl::domain_str(host) {
        let domain_url = format!("{}://{}", target_url.scheme(), domain);
        if unique_target.insert(&domain_url) {
            probes.push((domain_url.to_string(), "domain".to_string()));
        }
    }

    let mut results = Vec::new();
    for (url, probe_type) in probes {
        let result = probe(&url, &probe_type).await;
        let isup = result.status == "UP";
        results.push(result);
        if isup {
            break;
        }
    }

    let response = FinalResponse {
        requested_url: target_url.to_string(),
        results,
    };

    let cache_ttl: u32 = env
        .var("CACHE_TTL_SECONDS")
        .ok()
        .and_then(|s| s.to_string().parse().ok())
        .unwrap_or(600);

    let headers = Headers::new();
    headers.set("Cache-Control", &format!("max-age={cache_ttl}"))?;
    headers.set("X-Worker-Cache", "MISS")?;

    let mut response = Response::builder()
        .with_headers(headers)
        .from_json(&response)?;

    let cache_response = response.cloned()?;
    ctx.wait_until(async move {
        let _ = cache.put(cache_key, cache_response).await;
    });

    Ok(response)
}

async fn probe(url: &str, probe_type: &str) -> ProbeResult {
    let headers = Headers::new();
    headers.set("User-Agent", "up-down-workers/1.0").unwrap();

    let request = Request::new_with_init(
        url,
        &RequestInit {
            method: Method::Get,
            headers,
            ..RequestInit::default()
        },
    )
    .unwrap();

    let start_time = js_sys::Date::now();
    let result = match Fetch::Request(request).send().await {
        Ok(response) => {
            let status_code = response.status_code();
            let status = if (200..400).contains(&status_code) {
                "UP"
            } else {
                "DOWN"
            };

            ProbeResult {
                probe_type: probe_type.to_string(),
                url: url.to_string(),
                status: status.to_string(),
                status_code: Some(status_code),
                status_text: String::new(),
                duration_ms: None,
            }
        }
        Err(e) => ProbeResult {
            probe_type: probe_type.to_string(),
            url: url.to_string(),
            status: "DOWN".to_string(),
            status_code: None,
            status_text: format!("Network Error: {e}"),
            duration_ms: None,
        },
    };

    let duration = (js_sys::Date::now() - start_time) as u64;
    ProbeResult {
        duration_ms: Some(duration),
        ..result
    }
}
