use std::collections::HashMap;

pub fn repo_paths() -> HashMap<String, String> {
    [
        ("3p", "/Users/dan/tmp/3p"),
        ("bin", "/Users/dan/src/devenv/bin"),
        ("cli", "/Users/dan/src/temporalio/cli"),
        ("delta", "/Users/dan/src/delta"),
        ("devenv", "/Users/dan/src/devenv"),
        ("docker-builds", "/Users/dan/src/temporalio/docker-builds"),
        ("docker-compose", "/Users/dan/src/temporalio/docker-compose"),
        ("features-lite", "/Users/dan/src/temporalio/features-lite"),
        ("features", "/Users/dan/src/temporalio/features"),
        (
            "global-namespace-sdk-experiments",
            "/Users/dan/src/temporalio/global-namespace-sdk-experiments",
        ),
        ("log-view", "/Users/dan/src/temporalio/log-view"),
        ("mathematics", "/Users/dan/src/mathematics"),
        ("misc-python", "/Users/dan/src/misc-python"),
        ("notes", "/Users/dan/src/temporalio/notes"),
        ("nushell-config", "/Users/dan/src/devenv/nushell-config"),
        ("pm", "/Users/dan/src/pm"),
        ("proxy", "/Users/dan/src/devenv/tools/python"),
        ("python-demo", "/Users/dan/src/temporalio/python-demo"),
        ("samples-java", "/Users/dan/src/temporalio/samples-java"),
        (
            "sdk-core",
            "/Users/dan/src/temporalio/sdk-python/temporalio/bridge/sdk-core",
        ),
        (
            "sdk-dotnet-bridge",
            "/Users/dan/src/temporalio/sdk-dotnet/src/Temporalio/Bridge",
        ),
        ("sdk-dotnet", "/Users/dan/src/temporalio/sdk-dotnet"),
        ("sdk-go", "/Users/dan/src/temporalio/sdk-go"),
        ("sdk-java", "/Users/dan/src/temporalio/sdk-java"),
        (
            "sdk-python-bridge",
            "/Users/dan/src/temporalio/sdk-python/temporalio/bridge",
        ),
        ("sdk-python", "/Users/dan/src/temporalio/sdk-python"),
        (
            "sdk-typescript-bridge",
            "/Users/dan/src/temporalio/sdk-typescript/packages/core-bridge",
        ),
        ("sdk-typescript", "/Users/dan/src/temporalio/sdk-typescript"),
        ("server", "/Users/dan/src/temporalio/docker-builds/temporal"),
        ("shell-config", "/Users/dan/src/devenv/shell-config"),
        ("src", "/Users/dan/src"),
        ("swimlanesio-links", "/Users/dan/src/swimlanesio-links"),
        ("temporal-cli", "/Users/dan/src/temporalio/cli"),
        (
            "temporal-docker-builds",
            "/Users/dan/src/temporalio/docker-builds",
        ),
        (
            "temporal-docker-compose",
            "/Users/dan/src/temporalio/docker-compose",
        ),
        (
            "temporal-features-lite",
            "/Users/dan/src/temporalio/features-lite",
        ),
        ("temporal-features", "/Users/dan/src/temporalio/features"),
        (
            "temporal-log-view",
            "/Users/dan/src/temporalio/temporal-log-view",
        ),
        ("temporal-notes", "/Users/dan/src/temporalio/notes"),
        (
            "temporal-python-demo",
            "/Users/dan/src/temporalio/python-demo",
        ),
        (
            "temporal-samples-java",
            "/Users/dan/src/temporalio/samples-java",
        ),
        (
            "temporal-sdk-core",
            "/Users/dan/src/temporalio/sdk-python/temporalio/bridge/sdk-core",
        ),
        (
            "temporal-sdk-dotnet-bridge",
            "/Users/dan/src/temporalio/sdk-dotnet/src/Temporalio/Bridge",
        ),
        (
            "temporal-sdk-dotnet",
            "/Users/dan/src/temporalio/sdk-dotnet",
        ),
        ("temporal-sdk-go", "/Users/dan/src/temporalio/sdk-go"),
        ("temporal-sdk-java", "/Users/dan/src/temporalio/sdk-java"),
        (
            "temporal-sdk-python-bridge",
            "/Users/dan/src/temporalio/sdk-python/temporalio/bridge",
        ),
        (
            "temporal-sdk-python",
            "/Users/dan/src/temporalio/sdk-python",
        ),
        (
            "temporal-sdk-typescript-bridge",
            "/Users/dan/src/temporalio/sdk-typescript/packages/core-bridge",
        ),
        (
            "temporal-sdk-typescript",
            "/Users/dan/src/temporalio/sdk-typescript",
        ),
        (
            "temporal-server",
            "/Users/dan/src/temporalio/docker-builds/temporal",
        ),
        ("temporalite", "/Users/dan/src/temporalio/temporalite"),
        ("tonic", "/Users/dan/tmp/3p/tonic"),
        ("tower", "/Users/dan/tmp/3p/tower"),
        ("twitch", "/Users/dan/src/twitch"),
        ("vscode-emacs-mcx", "/Users/dan/src/vscode-emacs-mcx"),
        ("vscode-etc", "/Users/dan/src/vscode-etc"),
    ]
    .map(|(s, t)| (s.to_string(), t.to_string()))
    .into_iter()
    .collect()
}
