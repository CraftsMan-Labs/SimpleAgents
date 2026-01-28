import json

from simple_agents_py import Client


def main() -> None:
    client = Client(
        "openai",
        api_base="http://localhost:4000/v1",
        api_key="sk-skpHy0DGeJP3Bq7JExw_QQ",
    )
    model = "grok-code-fast-1"

    messages = [
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "Give me one project idea."},
        {"role": "assistant", "content": "Idea: build a local proxy tester."},
        {"role": "user", "content": "Give me a second idea in a different domain."},
    ]
    text = client.complete_messages(model, messages)
    print(text)

    schema = {
        "type": "object",
        "properties": {
            "ideas": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": {"type": "string"},
                        "one_liner": {"type": "string"},
                    },
                    "required": ["title", "one_liner"],
                    "additionalProperties": False,
                },
                "minItems": 3,
            }
        },
        "required": ["ideas"],
        "additionalProperties": False,
    }
    structured_messages = [
        {
            "role": "user",
            "content": "Give me three project ideas as JSON.",
        }
    ]
    json_text = client.complete_json_schema(
        model,
        structured_messages,
        schema,
        "project_ideas",
    )
    print(json.dumps(json.loads(json_text), indent=2))


if __name__ == "__main__":
    main()
