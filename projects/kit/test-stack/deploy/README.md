# Putting the test stack online

The stack runs on one free Oracle Cloud server. Everything here is free except an optional domain name.

## What only the owner can do

These need a person: accounts, a card check and passwords. The screens below were written from memory of Oracle's and Google's sites and may have moved; the words to look for should still be there.

**1. Oracle Cloud server (about 30 minutes)**

1. Sign up at [signup.cloud.oracle.com](https://signup.cloud.oracle.com). Oracle asks for a card to check identity; Always Free items aren't charged. Choose the home region with care, because it can't be changed and free servers only run there. Sydney or Melbourne are nearest to New Zealand.
2. Go to **Compute → Instances → Create instance**.
   - Image: **Canonical Ubuntu 24.04**.
   - Shape: **Ampere → VM.Standard.A1.Flex**, 2 OCPUs and 12 GB of memory. It should say "Always Free-eligible".
   - Networking: create a new virtual cloud network with a public subnet, and keep **Assign a public IPv4 address** on.
   - SSH keys: choose **Paste public keys** and paste the key you were given.
3. Create it and copy its **public IP address**. If it says "Out of capacity", try another availability domain or try again later; free ARM servers run short in busy regions.
4. Open the web ports: **Networking → Virtual cloud networks →** your network **→ Security → Default Security List → Add Ingress Rules**. Source `0.0.0.0/0`, protocol TCP, destination ports `80,443`.

Oracle can reclaim a free server that sits idle for a week, so keep it in use or expect to recreate it.

**2. A web address**

https needs a name, not a bare IP address. Either:

- **Free:** sign in at [duckdns.org](https://www.duckdns.org), add a subdomain such as `spare-test`, and set its IP to the server's. The address is `spare-test.duckdns.org`.
- **About US$10–15 a year:** buy a domain and add an `A` record pointing at the server's IP.

**3. Google sign-in client (about 10 minutes)**

1. At [console.cloud.google.com](https://console.cloud.google.com), create a project called `Spare test`.
2. Open **Google Auth Platform**. Under **Branding**, give the app a name and your email. Under **Audience**, choose **External** and add the Google address of everyone who will test; only they can sign in while the app is in testing.
3. Under **Clients → Create client**, choose **Web application**. Add this **Authorized redirect URI**, with your own address: `https://spare-test.duckdns.org/auth/callback`.
4. Copy the **Client ID** and **Client secret**.

## Deploying

Copy [stack.env.example](stack.env.example) to a file outside the repository and fill it in: the address, the Google client, and which Google account signs in as which test person. Then, from a checkout on your own machine:

```bash
projects/kit/test-stack/deploy/deploy.sh <server IP> <settings file>
```

The first run takes about 20 minutes, nearly all of it compiling on two cores. Running it again updates the server and leaves its data alone.

It installs TiKV, PD, RustFS, Caddy and the server as services, seeds the example data, and turns on a daily backup. Only Caddy faces the internet, on ports 80 and 443; it gets the https certificate by itself. Backups go to `/opt/spare-stack/backups`, one per weekday, which protects against mistakes but not against losing the server. Set `STACK_BACKUP_URL` in the settings file to send them elsewhere, such as Oracle's free Object Storage through its S3 address.

To let another tester in, add them to `STACK_PEOPLE` and to the Google list, then deploy again.

## Moving to three servers

No code changes. The local proof is `STACK_NODES=3`, which runs three PD and three TiKV copies and passes the whole checklist, with one TiKV killed mid-edit.

| Setting | One server | Three servers |
| --- | --- | --- |
| `STACK_PD` | `127.0.0.1:2379` | `a:2379,b:2379,c:2379` |
| `max-replicas` in `config/pd.toml` | `1` | `3`: every write on three machines |
| PD `--initial-cluster` | itself | all three |
| TiKV `--pd-endpoints` and `--addr` | loopback | the machines' private addresses |
| `STACK_FILES_URL` | `s3://spare-files` on RustFS | `s3://<bucket>` or `gs://<bucket>` |
| `AWS_ENDPOINT` | RustFS | removed, for real S3 |
| `STACK_BACKUP_URL` | a local folder | another provider's bucket |
| `tikv.toml` cache and threads | sized for 12 GB | sized for each machine |

Several copies of the web server can run behind a load balancer: TiKV's transaction on each project's `head` key keeps one order, and each copy passes on edits ordered by the others.

Before real use, the machines also need TLS between them (`tikv-client`, PD and TiKV all take certificates) and a private network. TiKV's own `br` tool should replace `test-stack backup` once a model outgrows memory.
