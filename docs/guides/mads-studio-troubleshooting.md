
# mads studio troubleshooting

Studio-specific problems. For generation and export issues, see [Troubleshooting](../howto/troubleshooting.md).

```text
error: cannot bind 127.0.0.1:4780: Address already in use
```

Another process holds the port. Use `--port` or stop the other process.

```text
Token inválido ou expirado. Copie o endereço impresso no terminal e tente de novo.
```

The token changes when the server restarts. Copy the new address from the terminal.

```text
~/.config/mads/secrets.toml é legível por outros usuários. Rode: chmod 600 ...
```

The key file is too open. Run the printed command.

```text
outra execução está usando este projeto
```

A job holds the project lock. Wait for it to finish or stop the server.

```text
business.toml is missing
```

The project directory has `project.json` but the init job did not write `business.toml`. Open the project and choose **Estudar o site de novo**. Check the init job log for the provider error. In a container, confirm that the project directory is inside the mounted `--root`. See [mads studio](mads-studio.md#create-a-project).

```text
ApiKeyEnvNotFound { env_name: "DEEPSEEK_API_KEY" }
```

The job had no key for the provider. Save it in **Conexões** and retry the job, or set the variable before starting `mads serve`. Saved keys are read when the job starts; a job that was already running when you saved the key still needs a retry. See [mads studio](mads-studio.md#connections).

The job page does not auto-refresh. When a job finishes, the status and hint update; reload the page to see the full error and the run links.
