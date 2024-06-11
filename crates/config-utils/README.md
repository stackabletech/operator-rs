# config-utils

This utility currently only supports filling your config with contents from environmental variables or files (called templating).

## Templating

Imagine the following `example.xml`:

```xml
cat > example.xml << 'EOF'
<credentials>
  <username>${env:EXAMPLE_USERNAME}</username>
  <password>${file:UTF-8:example-password}</password>
</credentials>
EOF
```

and the following `example-password`:

```bash
echo 'example-password <123>!' > example-password
```

You can run the following command to replace both placeholders:
```bash
export EXAMPLE_USERNAME=my-user

config-utils template example.xml
```

Afterwards the XML looks like

```xml
<credentials>
  <username>my-user</username>
  <password>example-password &lt;123&gt;!</password>
</credentials>
```

`config-utils` did the following steps to achieve the result:

1. Use the file extension to determine the file type (XML in this case). You can also specify the file type manually as a CLI argument.
2. Read the env var `EXAMPLE_USERNAME`, xml-escape it and insert it
3. Read the contents of the file `example-password`, xml-escape it and insert it

Please note that `config-utils` also supports nested templating, so the name of the file to read can come from an env var (or even another file as well).
This looks something like `${env:${env:ENV_TEST_PASSWORD_ENV_NAME}}`

## Currently supported file formats

1. `.properties` files
2. XML files
