#!/bin/bash

echo "Generating documentation..."
cargo doc --no-deps

echo "Copying generated documentation to doc directory..."
if [ -d "doc" ]; then
    rm -rf doc
fi
cp -r target/doc doc

echo "Creating index.html for redirection..."
cat <<EOL > doc/index.html
<!DOCTYPE html>
<html>
<head>
    <meta http-equiv="refresh" content="0; url=ogg_magic/index.html">
    <title>Redirecting...</title>
</head>
<body>
    <p>If you are not redirected automatically, follow this <a href="/ogg_magic/index.html">link to documentation</a>.</p>
</body>
</html>
EOL

echo "All steps completed successfully."