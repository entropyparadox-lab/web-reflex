import os
import re
from typing import Any, Callable, Dict, Optional


def redact_sensitive_html(html: str) -> str:
    """
    Sanitizes HTML snippets before sending to LLM healers by masking
    emails, phone numbers, credit cards, and sensitive input values.
    """
    # 1. Redact card numbers (4 blocks of 4 digits, or 13-19 digits)
    html = re.sub(
        r"\b(?:\d{4}[-\s]?){3}\d{4}\b",
        "[CARD_REDACTED]",
        html,
    )
    # 2. Redact emails
    html = re.sub(
        r"\b[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+\b",
        "[EMAIL_REDACTED]",
        html,
    )
    # 3. Redact phone numbers (e.g. +82-10-9876-5432, 010-1234-5678, 123-456-7890)
    html = re.sub(
        r"(?:\+?\d{1,3}[-.\s]?)?(?:\(?\d{2,4}\)?[-.\s])?\d{3,4}[-.\s]\d{4}\b",
        "[PHONE_REDACTED]",
        html,
    )
    # 4. Redact password/token/secret input values
    html = re.sub(
        r'(type=["\'](?:password|hidden)["\'][^>]*?value=["\'])[^"\']*(["\'])',
        r"\1[REDACTED]\2",
        html,
        flags=re.IGNORECASE,
    )
    html = re.sub(
        r'(name=["\'](?:password|pwd|token|secret|cvv|pin)["\'][^>]*?value=["\'])[^"\']*(["\'])',
        r"\1[REDACTED]\2",
        html,
        flags=re.IGNORECASE,
    )
    return html


def create_llm_healer(
    api_key: Optional[str] = None,
    model: str = "gpt-4o-mini",
    provider: str = "openai",
    base_url: Optional[str] = None,
    redact_pii: bool = False,
) -> Callable[[Dict[str, Any]], str]:
    """
    Creates an automated LLM healer callback for ReflexSession.
    Extracts the updated CSS or XPath selector for the failed action step.

    Supported providers:
      - 'openai': Uses api.openai.com or custom base_url
      - 'ollama': Uses local Ollama instance (default: http://127.0.0.1:11434)
      - 'local' / 'vllm': Uses OpenAI-compatible local LLM endpoint via base_url
    """
    key = (
        api_key
        or os.getenv("OPENAI_API_KEY")
        or os.getenv("ANTHROPIC_API_KEY")
    )

    def healer(context: Dict[str, Any]) -> str:
        failed_step = context.get("failed_step_id", "unknown")
        raw_html = context.get("html_snippet", "")
        graph_id = context.get("graph_id", "")

        html_snippet = redact_sensitive_html(raw_html) if redact_pii else raw_html

        prompt = f"""You are a Web Automation Selector Repair Agent.
A deterministic browser action failed at step: '{failed_step}'.
Graph ID: {graph_id}

Here is the HTML snippet of the active page:
```html
{html_snippet}
```

Task: Find the best, most resilient CSS selector for step '{failed_step}'.
Return ONLY the raw CSS selector string (e.g. #submit_btn or button.pay-now), with no markdown formatting or commentary."""

        import requests

        provider_lower = provider.lower()

        # 1. Ollama Native Endpoint
        if provider_lower == "ollama" and (not base_url or "/api/generate" in base_url):
            endpoint = base_url or "http://127.0.0.1:11434/api/generate"
            payload = {
                "model": model,
                "prompt": prompt,
                "stream": False,
            }
            resp = requests.post(endpoint, json=payload, timeout=20)
            resp.raise_for_status()
            text = resp.json().get("response", "").strip()
            return re.sub(r"^`+|`+$", "", text).strip()

        # 2. OpenAI or OpenAI-Compatible Endpoints (vLLM, Local Server, Ollama /v1, LiteLLM)
        if provider_lower in ("openai", "local", "vllm", "ollama"):
            if base_url:
                endpoint = base_url.rstrip("/")
                if not endpoint.endswith("/chat/completions"):
                    endpoint = f"{endpoint}/chat/completions"
            else:
                endpoint = "https://api.openai.com/v1/chat/completions"

            headers = {"Content-Type": "application/json"}
            if key:
                headers["Authorization"] = f"Bearer {key}"

            payload = {
                "model": model,
                "messages": [
                    {
                        "role": "system",
                        "content": "You are a precise CSS selector extraction engine. Output only the selector.",
                    },
                    {"role": "user", "content": prompt},
                ],
                "temperature": 0.0,
            }
            resp = requests.post(
                endpoint,
                headers=headers,
                json=payload,
                timeout=20,
            )
            resp.raise_for_status()
            text = resp.json()["choices"][0]["message"]["content"].strip()
            return re.sub(r"^`+|`+$", "", text).strip()

        return ""

    return healer