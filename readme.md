# Running a debug session

## Prerequisites
The simplest method to serve the frontend is using trunk. To install trunk using cargo run

```
cargo install --locked trunk
```
To install wasm you probably need to run
```
rustup target add wasm32-unknown-unknown
```

To build the backend the following dependencies are needed:
* `boost`
* `libuhd`

## Running
Start both backend and trunk using the suppliend scripts under development. First start the backend

```shell
./development/run-backend.sh
```

In a separate window run trunk to serve the frontend

```shell
./development/serve-frontend.sh
```

## Running with https
If you want to work with authentication you should enable https. Otherwise password will not be encrypted in transit and redirect will not work properly (identity server will typically only allow redirect to https address). To run salsa with https a little more work is needed. It will also not be possible to use trunk.

The guide below is based on https://www.splitbrain.org/blog/2017-08/10-homeassistant_duckdns_letsencrypt

1. Set up dns name for salsa (we will issue cert to this DNS)

    1. Go to duckdns.org and login
    2. Create a new domain and set the ip to the machine you plan to serve salsa from. If you are developing it is probably a good idea to use an internal IP here that you can reach, e.g., use 192.168.0.... In this guide the domain will be salsa, replace with the name you selected.

2. Download dehydrated

    ```shell
    git clone git@github.com:dehydrated-io/dehydrated.git
    cd dehydrated
    ```

3. Configure dehydrated

    1. Enter the dehydrated directory
        ```shell
        cd dehydrated
        ```
    2. Set domain to the one created above.
        ```shell
        echo "salsa.duckdns.org" >> domains.txt
        ```
    3. Create a file called `config` with
        ```
        CHALLENGETYPE="dns-01"
        HOOK="${BASEDIR}/hook.sh"
        ```
    4. Create the `hook.sh` refenced above. This will tell dehydrated how to access your duckdns domain

        ```
        #!/usr/bin/env bash
        set -e
        set -u
        set -o pipefail

        domain="salsa"
        token="your-duckdns-token"

        case "$1" in
            "deploy_challenge")
                curl "https://www.duckdns.org/update?domains=$domain&token=$token&txt=$4"
                echo
                ;;
            "clean_challenge")
                curl "https://www.duckdns.org/update?domains=$domain&token=$token&txt=removed&clear=true"
                echo
                ;;
            "deploy_cert")
                echo "Update certificate"
                echo
                ;;
            "unchanged_cert")
                ;;
            "startup_hook")
                ;;
            "exit_hook")
                ;;
            *)
                echo Unknown hook "${1}"
                exit 0
                ;;
        esac
        ```

    5. Enter token and domain in the file. Token can be copied from duckdns.org page.
    6. Register for certificate

        ```shell
        ./dehydrated --register  --accept-terms
        ```

4. Generate the certificate. This command can be added to cron as well if you want it to be automatically be refreshed.

    ```
    ./dehydrated -c
    ```

5. Add or update `development/env.sh` with path to the certificate and private key from dehydrated. Change salsa to the name you opted for.

    ```bash
    echo "export KEY_FILE_PATH=<path to dehydrated>/certs/salsa.duckdns.org/privkey.pem" >> ./development/env.sh
    echo "export CERT_FILE_PATH=<path to dehydrated>/certs/salsa.duckdns.org/fullchain.pem" >> ./development/env.sh
    ```

6. Run backend with frontend script

    ```shell
    ./development/run-backend-with-frontend.sh
    ```

    Should log that frontend is served and the certificate/key used for https.
