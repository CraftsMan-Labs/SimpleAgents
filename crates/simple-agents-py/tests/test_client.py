import pytest


def test_unknown_provider_raises():
    import simple_agents_py

    with pytest.raises(Exception) as excinfo:
        simple_agents_py.Client("unknown")
    assert "Unknown provider" in str(excinfo.value)


def test_missing_openai_env_raises(monkeypatch):
    import simple_agents_py

    monkeypatch.delenv("OPENAI_API_KEY", raising=False)
    with pytest.raises(Exception) as excinfo:
        simple_agents_py.Client("openai")
    assert "OPENAI_API_KEY" in str(excinfo.value)
