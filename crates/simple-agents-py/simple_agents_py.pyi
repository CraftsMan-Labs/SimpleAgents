class Client:
    def __init__(
        self,
        provider: str,
        api_key: str | None = None,
        api_base: str | None = None,
    ) -> None: ...

    def complete(
        self,
        model: str,
        prompt: str,
        max_tokens: int | None = None,
        temperature: float | None = None,
    ) -> str: ...

    def complete_messages(
        self,
        model: str,
        messages: list[dict[str, object]],
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
    ) -> str: ...

    def complete_json(
        self,
        model: str,
        messages: list[dict[str, object]],
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
    ) -> str: ...

    def complete_json_schema(
        self,
        model: str,
        messages: list[dict[str, object]],
        schema: dict[str, object],
        schema_name: str,
        max_tokens: int | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        strict: bool = True,
    ) -> str: ...
