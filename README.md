# up-down-workers

A Cloudflare Worker that probe whether a website (HTTP/HTTPS service) is "up" or "down", with caching and simple API key authentication.

## Development

### Environment Variables
- **secret** `API_KEY`: The secret key required in the `x-api-key` header.
- `CACHE_TTL_SECONDS`: (optional) How long to cache probe results (default: 600 seconds).

Variables are set in [`wrangler.toml`](wrangler.toml), secrets for local development are set in `.dev.vars`.

Start a local development server:

```sh
npx wrangler dev
```

This will start a local server at http://localhost:8787.

## Deployment

```sh
npx wrangler secret put API_KEY
npx wrangler deploy
```

## Example Usage

### GET Example

```sh
curl -H "x-api-key: 8Gvyu7uwc7TI1duHNzL839LpaaihCivl" \
  "http://localhost:8787/?url=instagram.com"
```

### POST Example

```sh
curl -X POST http://localhost:8787 \
  -H "x-api-key: 8Gvyu7uwc7TI1duHNzL839LpaaihCivl" \
  -H "Content-Type: application/json" \
  -d '{"url": "https://www.instagram.com"}'
```

### Example Response

```json
{
  "requested_url": "https://instagram.com/",
  "results": [
    {
      "type": "host",
      "url": "https://instagram.com",
      "status": "UP",
      "status_code": 200,
      "status_text": ""
    }
  ]
}
```

### Result

- `type`: Either `host` or `domain`.
    - Note: The `domain` type is a fallback that is only checked when the host target is `DOWN`.
- `url`: Checked URL.
- `status`: Either `UP` or `DOWN`
- `status_code`: HTTP status code from the checked URL.
- `status_text`: If any, it's currently used as an error message.

### Caching

Probe results are cached for the duration specified in `CACHE_TTL_SECONDS`. The `X-Worker-Cache` response header indicates whether the request was a cache `HIT` or `MISS`.

### Error Responses

- `401 Unauthorized`: Missing or incorrect API key.
- `400 Bad Request`: Invalid input or non-existent domain.
- `405 Method Not Allowed`: Only GET and POST are supported.
