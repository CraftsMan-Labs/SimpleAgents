from simple_agents_py import Client


def main() -> None:
    client = Client(
        "openai",
        api_base="http://localhost:4000/v1",
        api_key="sk-skpHy0DGeJP3Bq7JExw_QQ",
    )
    text = client.complete("gpt-4.1", "Give me three project ideas.")
    print(text)


if __name__ == "__main__":
    main()
