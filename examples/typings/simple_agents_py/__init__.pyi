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
