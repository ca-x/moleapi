# Explicit PECL HTTP QA tools

These are consumer validation tools, not dependencies installed or loaded by MoleAPI.
The current native evidence uses PHP8.5.11 NTS/Linux x86_64, PECL HTTP4.3.1,
raphf2.0.2, libcurl8.22.0 and pear/http_request2 2.8.0. PHP's matching development
headers, phpize/php-config, C compiler/autoconf/make and extension library headers
are required for source builds. Other ABI/platform combinations need their own
build and actual-run evidence.

## Isolated SDK procedure

1. Obtain the official release tarballs named in pecl-source-lock.json and check
   their recorded SHA256. Official HTTPS release metadata identifies versions and
   compressed sizes. The official package.VERSION.xml resources contain per-file
   MD5 attributes: all21 raphf and320 HTTP source files matched in this validation.
   Release metadata's `m` element is the maintainer, not an archive checksum.
2. Keep the selected PHP binary unchanged. Copy its matching include/php and
   lib/php/build trees into a private SDK. Copy phpize/php-config scripts too;
   relocate their prefix and php-config's hardcoded extension_dir to that SDK.
   Do not change host /usr, php.ini or system packages.
3. In pristine raphf2.0.2 source, run the relocated phpize, configure with
   --with-php-config=/absolute/private-sdk/bin/php-config --enable-raphf, then make.
   Stage its public headers under the private SDK's include/php/ext/raphf and its
   module in the SDK's extension directory. No global make install is needed.
4. The private php-config's --php-binary must resolve to the selected PHP runtime
   with raphf loaded while configuring HTTP. A private launcher passing
   -d extension=/absolute/raphf.so provides this. In pristine HTTP4.3.1 source, run
   phpize, configure with that php-config and --with-http, then make. No extension
   source patches were needed. Preserve compiler/configure logs and module hashes.
5. Place the resulting raphf.so/http.so in an explicit directory. Install the
   committed Composer lock in a separate directory with scripts/plugins disabled.
   Use a base PHP launcher with ext-curl available but without these PECL modules
   preloaded; the driver loads them itself, in dependency order:

```sh
python3 tools/snippets/verify-php.py --php /absolute/base-php --node /absolute/node --dependencies /absolute/composer-project --extensions /absolute/pecl-modules --output /absolute/fresh-output
```

The driver records both module hashes and actual loaded versions, along with the
PHP binary, engine, Composer lock, saved request and original snippet hashes.
Its default5 clients ×18 profiles execute original trusted snippets through native
clients; Node HTTP observes framing/bytes and stdlib MIME parses multipart. It adds
only autoload, actual Laravel facade bootstrap, PHP tag context and assertions.
HTTP_Request2 uses its default Socket adapter; PECL HTTP uses its curl driver.
No HTTP fakes, alternate transports, body rewriting or user-imported code run.

Use --clients curl,guzzle,laravel,httprequest2 without --extensions to validate only
those four clients in a base PHP environment. PECL checks require actual modules;
a missing extension is a prerequisite error, not a skipped passing case.
