import os
import re
from typing import Any, Callable, Dict, Optional


def create_llm_healer(
    api_key: Optional[str] = None,
    model: str = "gpt-4o-mini",
    provider: str = "openai",
) -> Callable[[Dict[str, Any]], str]:
    """
    Creates an automated LLM healer callback for ReflexSession.
    Extracts the updated CSS or XPath selector for the failed action step.
    """
    key = (
        api_key
        or os.getenv("OPENAI_API_KEY")
        or os.getenv("ANTHROPIC_API_KEY")
    )

    def healer(context: Dict[str, Any]) -> str:
        failed_step = context.get("failed_step_id", "unknown")
        html_snippet = context.get("html_snippet", "")
        graph_id = context.get("graph_id", "")

        prompt = f"""You are a Web Automation Selector Repair Agent.
A deterministic browser action failed at step: '{failed_step}'.
Graph ID: {graph_id}

Here is the HTML snippet of the active page:
```html
{html_snippet}
```

Task: Find the best, most resilient CSS selector for step '{failed_step}'.
Return ONLY the raw CSS selector string (e.g. #submit_btn or button.pay-now), with no markdown formatting or commentary."""

        if provider == "openai":
            import requests

            headers = {
                "Authorization": f"Bearer {key}",
                "Content-Type": "application/json",
            }
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
                "https://api.openai.com/v1/chat/completions",
                headers=headers,
                json=payload,
                timeout=10,
            )
            resp.raise_for_status()
            text = resp.json()["choices"][0]["message"]["content"].strip()
            # Clean possible markdown ticks
            text = re.sub(r"^`+|`+$", "", text).strip()
            return text

        return ""

    return healer
