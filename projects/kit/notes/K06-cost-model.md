# K06: Cost model and open cost ledger

*Piece K06, shared kit. Written 2026-10-19. An estimate for planning, not a quote. The numbers are in [cost-model.json](../cost-model.json) and on the [costs page](https://spareware.app/costs.html).*

## The answer in plain words

All figures are US dollars, excluding sales tax. Each country adds its own (GST, VAT or sales tax). The middle case is shown first, with the low to high range in brackets. Card fees use Stripe in New Zealand as the example; they differ by country.

| At 10,000 a month | Running cost | Card fee (example) | At-cost fee | What the big platform takes |
| --- | --- | --- | --- | --- |
| One US$15 ride | US$0.17 (0.08 to 0.42) | US$0.57 | **US$0.75, 5% of the fare** | Uber: 28% illustrated, US$4.20, plus a rider fee (unverified) |
| One US$25 food order | US$0.17 (0.08 to 0.42) | US$0.84 | **US$1.02, 4% of the order** | Uber Eats: 30%, US$7.50 |
| One US$300 stay booking | US$0.68 (0.15 to 6.46) | US$8.27 | **US$8.95, 3% of the booking** | Airbnb: 15.5%, US$46.50 |
| One social member, a month | US$0.41 (0.11 to 5.02) | none | **US$0.41** | No researched comparison |
| One office user, a month | US$2.07 (0.55 to 8.36) | not costed | **US$2.07** | Microsoft 365 Business Basic: US$6.40 (NZ price) |

- **The card fee is most of the fee.** From 10,000 transactions a month up, the card fee is about 75 to 98% of the at-cost fee for rides, orders and bookings (middle case).
- **Fixed costs are mostly people.** Each app needs about US$700 to US$4,000 a month whatever its size (middle about US$1,600 to US$1,800). A small team for support, moderation and bookkeeping is US$660 to US$2,900 of that; servers and services are US$55 to US$1,100.
- **When fixed costs stop mattering.** Fixed costs add less than 10 US cents per ride, order, booking or member once an app has about **7,000 to 40,000 a month** (middle about 16,000 to 18,000). The servers and services alone pass that point at 500 to 11,000 a month.
- **Below about 1,000 a month the fee is high.** At 100 rides a month the fixed costs alone come to about US$16 a ride. A new app needs to share a deployment with others, or have its first months covered, until it reaches a few thousand transactions.
- **Where we would not beat an incumbent:** Spare Market on small sales. On a US$40 sale the at-cost fee is about US$1.56 (3.9%), above Trade Me's 2.19% seller fee (US$0.88), though Trade Me buyers also pay NZ$0.99 to NZ$4.99. The card fee is the cause. Spare Office's high case (US$8.36 a user) is also above Microsoft 365 Business Basic.

## How the model works

**At-cost fee per unit = fixed monthly cost / monthly volume + variable cost per unit.** Fixed means the app's own servers, the shared kit's common items (backup, bookkeeping software, monitoring, sign-in, app store fee) and a small team. The card or bank fee is passed through on top, per transaction. This matches the rule in the root [ARCHITECTURE.md](../../../ARCHITECTURE.md): the fee is last period's running cost divided by the jobs, with no margin.

- **Low:** the cheapest researched setup with light use: small rented servers (OVHcloud Sydney or Auckland, Hetzner Europe), Cloudflare R2 storage, open-source maps, search and chat on our own servers, Amazon SES email.
- **High:** the dearest researched setup with heavy use: AWS Auckland or Azure New Zealand North plus paid services (Mapbox, Algolia, Stream Chat, Postmark, Twilio, Sumsub, AWS MediaConvert and Rekognition).
- **Middle:** for each part, the geometric mean of its low and high values, the usual middle for estimates that span 10 to 1,000 times. A part that costs nothing in the low case (because we run it ourselves) also costs nothing in the middle case. It is not a priced setup.
- **People:** a small team, 10 hours a week at the NZ minimum wage (low) to 35 hours a week at the NZ Living Wage (high), with holiday pay, KiwiSaver and ACC on-costs: US$662 to US$2,892 a month, middle US$1,384. Wages differ by country.
- **Pilot size:** the research sized servers and the team for 1,000 to 5,000 members and up to about 10,000 jobs a month. Above that the team must grow; how fast is not researched, so the people line is too low at large volumes.
- **Exchange rates:** ECB reference rates of 16 Oct 2026 ([source](https://api.frankfurter.dev/v1/2026-10-16?from=USD&to=NZD,AUD,GBP,EUR,CAD), the latest published when opened on 19 Oct): US$1 = NZ$1.7656 = A$1.437 = £0.74989 = €0.88865 = C$1.4161. Prices quoted in EUR, AUD or NZD were converted at these rates. The research added 15% NZ GST to every price; it is removed here.

Our own assumptions, not research: the example amounts (ride US$15, order US$25, booking US$300, job US$500, sale US$40, rent US$300 a week); 10 active users a month for each list-app transaction, so the costs of people who browse but do not buy are counted; a 12-month tenancy for Spare Rent. Each app is costed as if it ran alone; Ride and Food can share one deployment, and so can the five list apps, which lowers each one's fixed cost.

## Costs by family

Fixed costs are US$ a month and include the kit's common items. Variable costs are US$ per unit. Low, middle and high.

### Spare Ride and Spare Food (per ride, per order)

| Part | Fixed low | Fixed middle | Fixed high | Per unit low | Per unit middle | Per unit high |
| --- | --- | --- | --- | --- | --- | --- |
| Servers and services | 53 | 170 | 542 | 0 | 0 | 0.0005 |
| Maps (routing server, tiles) | 10 | 40 | 153 | 0.0001 | 0.0002 | 0.0005 |
| Messages (texts, email) | 1.15 | 4.31 | 16 | 0.0036 | 0.014 | 0.056 |
| **Total** | **65** | **214** | **711** | **0.0036** | **0.014** | **0.057** |
| Card fee, US$15 ride | | | | 0.57 | 0.57 | 0.82 |
| Card fee, US$25 order | | | | 0.83 | 0.84 | 1.21 |

Low: three OVHcloud VPS-2 servers in Sydney (app, database, routing), open map tiles on R2, texts by ClickSend at its NZ$20 top-up rate. High: three AWS c7g.xlarge in Auckland, Twilio texts, Postmark email. Routing, address search, live location and chat run on our own servers, so they add almost nothing per ride. Food adds menu photos, which cost a fraction of a cent. Original research (NZ$ incl GST): NZ$0.0062 (corrected to NZ$0.0074) to NZ$0.1147 per ride; fixed NZ$80.22 (OVH) to NZ$981.77 (AWS).

Paid maps are not in the plan (the kit map uses open map data). If they were used, a ride would cost US$0.02 to US$0.29 more with Mapbox, or US$0.06 to US$0.67 more with Google Maps Platform.

### Spare Stay, Home, Rent, Work and Market (per booking, listing, tenancy month, job, sale)

Fixed, all five: US$55 (low), US$176 (middle), US$1,092 (high) a month. Low is two OVHcloud servers in Sydney and R2; high is two Azure servers in New Zealand North plus Stream Chat's US$499 plan, Sumsub's US$149 minimum and the kit's paid services.

| App and unit | Per unit low | Per unit middle | Per unit high | Payment fee low | Middle | High |
| --- | --- | --- | --- | --- | --- | --- |
| Stay, per booking | 0.076 | 0.53 | 6.06 | 8.12 | 8.27 | 19.29 |
| Home, per listing to sale | 0.48 | 1.44 | 7.92 | none (no money held) | none | none |
| Rent, per tenancy month | 0.034 | 0.12 | 0.87 | 0 (bank transfer) | 9.82 (direct debit) | 14.81 |
| Work, per job | 0.076 | 0.52 | 6.01 | 13.42 | 13.67 | 16.19 |
| Market, per sale | 0.006 | 0.16 | 5.80 | 1.23 | 1.25 | 2.62 |

The high cases are mostly browsing: at 10 active users per transaction, paid maps, search, two texts and 30 emails per user a month add up to US$5.56. Home and Rent include a one-off ID check for the seller or landlord (Didit US$0.33 low, Sumsub US$1.35 high). Original research (NZ$ incl GST): Stay NZ$0.13 to 0.91 per booking, Home NZ$0.93 to 4.82, Rent letting NZ$0.80 to 4.36, Work NZ$0.13 to 0.93, Market NZ$0.001 to 0.49, plus NZ$0.0011 to 1.31 per active user.

### Spare Social (per active member a month)

| Part | Fixed low | Middle | High | Per member low | Middle | High |
| --- | --- | --- | --- | --- | --- | --- |
| Servers, video and storage | 350 | 448 | 574 | 0.012 | 0.21 | 3.75 |
| Messages (account emails, texts) | 1.15 | 4.31 | 16 | 0.0004 | 0.0095 | 0.23 |
| Moderation | 0 | 0 | 0 | 0 | 0 | 0.70 |
| **Total** | **351** | **452** | **590** | **0.013** | **0.22** | **4.67** |

Low: a member watches 3 hours a month from R2 (free bandwidth), with servers in OVHcloud's Auckland zone and a Hetzner box for transcoding and a self-hosted moderation model. High: 20 hours a month through AWS CloudFront, AWS transcoding and Rekognition moderation. Watching video is the main cost: US$0.15 an hour on CloudFront, almost nothing from R2. Original research: NZ$0.027 to NZ$9.04 (corrected NZ$9.02) per member.

### Spare Office (per active user a month)

Fixed US$263 (low, Hetzner Singapore) to US$750 (high, AWS Auckland). Per user US$0.45 (low) to US$7.99 (high), middle US$1.88. It covers 10 GB of files with versions, live co-editing, email and video calls, sized with the openDesk scaling guide. Video calls and co-editing compute are the main costs. Original research: NZ$0.92 to NZ$15.77 per user.

### Spare Construct and Spare Studio (per active user a month)

Fixed US$122 (low, Hetzner Europe) to US$540 (high, AWS Auckland). Per user: Construct US$0.10 to US$17.51 (middle US$1.32); Studio US$0.12 to US$38.09 (middle US$2.18). The tools run on the person's own computer, so the cost is storage, downloads and a share of a sync server. Downloads are the main cost on big clouds, which is why the low case uses providers with free or cheap bandwidth. Optional AI is not in the fee (bring your own model costs nothing; light use US$0.30, heavy use US$30 a member a month at Claude API prices). Original research: Construct NZ$0.20 to NZ$35.16, Studio NZ$0.25 to NZ$77.05.

### Spare OS (per installed computer a month)

Fixed US$106 (low, OVHcloud Auckland mirror and build box) to US$365 (high, AWS Auckland build server plus the kit's paid services). Per computer US$0.0001 (low, updates from R2 with free bandwidth) to US$0.82 (high, 7.2 GB a month through AWS). Original research: NZ$0.0002 to NZ$1.588 per computer.

### The shared kit and the people layer

Common items on top of each app's own servers: off-site backup (US$0.70), bookkeeping software (US$21 to US$50, example Xero NZ), company filing (US$2.35), app store fee (US$8.25), a text sender number (US$1.15), and in the high case monitoring (US$48), DDoS protection (US$25), hosted sign-in (US$100) and an email plan (US$15). Total US$33 to US$251 a month. Member apps also carry account messages: US$0.0004 to US$0.23 a member a month.

People layer (separate): US$662 (low), US$1,384 (middle), US$2,892 (high) a month. Legal advice and an outside accountant are not included (no public rates found).

## Break-even: the at-cost fee as volume grows

Running costs per unit (servers and services, maps, messages, checks and the team), middle case with low to high in brackets, US$. Fee = fixed / volume + variable.

| App, per unit | 100 a month | 1,000 | 10,000 | 100,000 | 1,000,000 |
| --- | --- | --- | --- | --- | --- |
| Ride, per ride | 16.0 (7.27 to 36.1) | 1.61 (0.73 to 3.66) | 0.17 (0.08 to 0.42) | 0.030 (0.011 to 0.093) | 0.016 (0.004 to 0.061) |
| Food, per order | 16.0 (7.27 to 36.1) | 1.61 (0.73 to 3.66) | 0.17 (0.08 to 0.42) | 0.030 (0.011 to 0.093) | 0.016 (0.004 to 0.061) |
| Stay, per booking | 16.1 (7.24 to 45.9) | 2.09 (0.79 to 10.0) | 0.68 (0.15 to 6.46) | 0.54 (0.083 to 6.10) | 0.53 (0.077 to 6.06) |
| Home, per listing | 17.0 (7.64 to 47.8) | 3.00 (1.19 to 11.9) | 1.59 (0.55 to 8.32) | 1.45 (0.48 to 7.96) | 1.44 (0.48 to 7.92) |
| Rent, per tenancy month | 15.7 (7.20 to 40.7) | 1.68 (0.75 to 4.85) | 0.27 (0.11 to 1.26) | 0.13 (0.041 to 0.91) | 0.12 (0.035 to 0.87) |
| Work, per job | 16.1 (7.24 to 45.9) | 2.08 (0.79 to 10.0) | 0.68 (0.15 to 6.41) | 0.54 (0.083 to 6.05) | 0.52 (0.076 to 6.02) |
| Market, per sale | 15.8 (7.17 to 45.6) | 1.72 (0.72 to 9.79) | 0.31 (0.078 to 6.20) | 0.17 (0.013 to 5.84) | 0.16 (0.007 to 5.80) |
| Social, per member | 18.6 (10.1 to 39.5) | 2.06 (1.02 to 8.15) | 0.41 (0.11 to 5.02) | 0.24 (0.023 to 4.71) | 0.23 (0.014 to 4.67) |
| Office, per user | 20.1 (9.70 to 44.4) | 3.71 (1.38 to 11.6) | 2.07 (0.55 to 8.36) | 1.90 (0.46 to 8.03) | 1.89 (0.45 to 8.00) |
| Construct, per user | 17.7 (7.94 to 51.8) | 2.96 (0.88 to 20.9) | 1.49 (0.18 to 17.9) | 1.34 (0.11 to 17.5) | 1.33 (0.10 to 17.5) |
| Studio, per user | 18.6 (7.96 to 72.4) | 3.82 (0.91 to 41.5) | 2.35 (0.20 to 38.4) | 2.20 (0.13 to 38.1) | 2.18 (0.13 to 38.1) |
| OS, per computer | 15.8 (7.68 to 33.4) | 1.59 (0.77 to 4.08) | 0.17 (0.077 to 1.15) | 0.025 (0.008 to 0.86) | 0.011 (0.001 to 0.83) |
| *of which the team* | 13.8 (6.62 to 28.9) | 1.38 (0.66 to 2.89) | 0.14 (0.066 to 0.29) | 0.014 (0.007 to 0.029) | 0.001 (0.001 to 0.003) |

Add the card or bank fee for transaction apps (middle case, Stripe NZ example): ride US$0.57, order US$0.84, booking US$8.27, job US$13.67, sale US$1.25, rent US$9.82 a month by direct debit. Total middle at-cost fee at 10,000 a month: ride US$0.75, order US$1.02, booking US$8.95, job US$14.35, sale US$1.56, tenancy month US$10.09, listing US$1.59.

The team row is the reason the fee keeps falling past 10,000 a month. It is too low there: a bigger app needs a bigger team, which this model does not yet cost.

## What the open cost ledger publishes each month

The ledger is the kit's monthly public record of what each app cost and what the fee paid for. It follows the same parts as this model, so plan and actual can be compared line by line. For each app and each month:

| Field | What it shows |
| --- | --- |
| Volume | Rides, orders, bookings, jobs, sales, tenancy months or active members |
| Servers and services | Each provider bill: servers, storage, bandwidth, sign-in, monitoring, backups, bookkeeping software, app store fees |
| Maps, messages, ID checks and moderation | Each bill, with the units used (texts sent, emails, checks run, map calls) |
| People | Hours worked by role and total pay with on-costs; no names unless the person agrees |
| Card and bank fees | Total passed through, the provider's rates, and the share of transaction value |
| Shared costs | Kit costs split across apps, and the rule used to split them |
| Fee | The fee charged, the fee this month's costs imply, and any surplus or shortfall carried into next month's fee |
| Against the plan | Each line next to this model's low, middle and high, so surprises are visible |
| Sign-off | Who checked it against the bills, and when |

It is published as open data (JSON and CSV, in the shape of `cost-model.json`) and as a page like the costs page. Bills are summarised by provider and amount; account numbers and personal data are left out.

## Compared honestly with what incumbents take

From the root ARCHITECTURE.md and each program's map (researched 2026-10-18; New Zealand examples). The Spareware figure is the middle at-cost fee at 10,000 a month including the card fee.

| App | Incumbent's cut | On the example | Spareware at cost | Notes |
| --- | --- | --- | --- | --- |
| Ride | Uber: 28% service fee illustrated in its NZ driver guide, plus a rider fee ([source](https://www.uber.com/nz/en/blog/understanding-earnings-post-1-april-2024/)) | US$4.20 on US$15 | US$0.75 (5%) | The real current rate is unverified; the rider fee is not counted |
| Food | Uber Eats: 30% with Uber delivery, 16% own staff, 8% pickup ([source](https://merchants.ubereats.com/nz/en/pricing/)) | US$7.50 on US$25 | US$1.02 (4%) | Customer delivery fees not counted |
| Stay | Airbnb: 15.5% for most hosts ([source](https://www.airbnb.co.nz/help/article/1857)) | US$46.50 on US$300 | US$8.95 (3%) | |
| Home | Agents: commonly 3 to 4% plus GST, minimums around NZ$11,000 ([source](https://www.calculate.co.nz/flat-fee-vs-commission-calculator.php)) | 3 to 4% of the sale price | US$1.59 per listing | Agent figure unverified; Spare Home holds no money, so no card fee |
| Rent | Property managers: 7 to 10% of rent plus a letting fee of about a week's rent ([source](https://www.propertybrokers.co.nz/news/how-much-does-property-management-cost-in-new-zealand)) | US$91 to US$130 a month on US$300 a week | US$10.09 a month (0.8%) | Mostly the direct-debit fee; US$0.27 if rent goes by plain bank transfer |
| Work | Fiverr: reportedly 20% from sellers plus 5.5% from buyers ([source](https://vaultleap.com/blog/fiverr-fees-explained-2026)); Builderscrack: NZ$79 to NZ$149 plus GST a month for leads | US$100 on US$500 | US$14.35 (2.9%) | Fiverr from secondary sources |
| Market | Trade Me: sellers 2.19%, buyers NZ$0.99 to NZ$4.99 on purchases over NZ$20 ([source](https://consumer.org.nz/shopping/online-shopping/the-fine-print-whats-behind-trade-mes-move-to-scrap-success-fees)) | US$0.88 on US$40 (seller side) | US$1.56 (3.9%) | **Higher than Trade Me's seller fee**, because of the card fee on a small sale |
| Office | Microsoft 365 Business Basic: NZ$11.30 per user a month, paid yearly, ex GST ([source](https://www.microsoft.com/en-nz/microsoft-365/business/microsoft-365-plans-and-pricing)) | US$6.40 a user | US$2.07 a user (US$0.55 to US$8.36) | The high case is above Microsoft's price |
| Social, Construct, Studio, OS | Not researched | | | |

Where Spareware comes out lower, it is because nothing is taken on top of the running costs; the card fee is a large share of Spareware's own fee in every case. Where the sale is small (Market) or the use is heavy (Office high, Studio high), the at-cost fee can be close to or above what an incumbent charges.

## Sources

Every price comes from the checked research of 2026-10-18, with every checker correction applied (listed in `cost-model.json`). Main sources:

- Exchange rates: [ECB via Frankfurter](https://api.frankfurter.dev/v1/2026-10-16?from=USD&to=NZD,AUD,GBP,EUR,CAD)
- Card and bank fees: [Stripe NZ pricing](https://stripe.com/en-nz/pricing), [Stripe NZ Connect pricing](https://stripe.com/en-nz/connect/pricing)
- Servers: [OVHcloud VPS catalogue](https://ca.api.ovh.com/1.0/order/catalog/public/vps?ovhSubsidiary=AU), [OVHcloud Public Cloud prices](https://www.ovhcloud.com/en-au/public-cloud/prices/), [Hetzner Cloud](https://www.hetzner.com/cloud/regular-performance/), [Hetzner Object Storage](https://www.hetzner.com/storage/object-storage/), [AWS EC2](https://aws.amazon.com/ec2/pricing/on-demand/), [AWS S3 Auckland](https://pricing.us-east-1.amazonaws.com/offers/v1.0/aws/AmazonS3/current/ap-southeast-6/index.json), [AWS data transfer Auckland](https://pricing.us-east-1.amazonaws.com/offers/v1.0/aws/AWSDataTransfer/current/ap-southeast-6/index.json), [AWS CloudFront](https://aws.amazon.com/cloudfront/pricing/pay-as-you-go/), [Azure retail prices](https://prices.azure.com/api/retail/prices?$filter=armRegionName%20eq%20'newzealandnorth'%20and%20serviceName%20eq%20'Virtual%20Machines')
- Storage and media: [Cloudflare R2](https://developers.cloudflare.com/r2/pricing/), [Cloudflare Images](https://developers.cloudflare.com/images/pricing/), [Cloudflare Stream](https://developers.cloudflare.com/stream/pricing/), [Backblaze B2](https://www.backblaze.com/cloud-storage/pricing), [AWS MediaConvert](https://pricing.us-east-1.amazonaws.com/offers/v1.0/aws/AWSElementalMediaConvert/current/ap-southeast-2/index.json), [AWS Rekognition](https://pricing.us-east-1.amazonaws.com/offers/v1.0/aws/AmazonRekognition/current/ap-southeast-2/index.json), [openDesk scaling guide](https://docs.opendesk.eu/operations/scaling/)
- Maps, search and chat: [Mapbox](https://www.mapbox.com/pricing), [Google Maps Platform](https://developers.google.com/maps/billing-and-pricing/pricing), [Algolia](https://www.algolia.com/pricing), [Stream Chat](https://getstream.io/chat/pricing/)
- Messages: [ClickSend NZ](https://www.clicksend.com/nz/pricing/nz/), [Twilio](https://www.twilio.com/en-us/sms/pricing/nz), [Amazon SES](https://aws.amazon.com/ses/pricing/), [Postmark](https://postmarkapp.com/pricing)
- ID checks: [Didit](https://didit.me/pricing), [Didit pricing change](https://didit.me/blog/new-pricing-november-2026/), [Sumsub](https://sumsub.com/pricing/)
- Kit services: [Grafana](https://grafana.com/pricing/), [Sentry](https://sentry.io/pricing/), [Zitadel](https://zitadel.com/pricing), [Cloudflare plans](https://www.cloudflare.com/plans/network-cdn/), [Apple Developer Program](https://developer.apple.com/programs/), [Xero NZ](https://www.xero.com/nz/pricing-plans/), [NZ Companies Office fees](https://companies-register.companiesoffice.govt.nz/help-centre/managing-your-online-account/schedule-of-fees/)
- People: [NZ minimum wage](https://www.employment.govt.nz/news-and-updates/minimum-wage-is-increasing-on-1-april-2026), [NZ Living Wage](https://www.livingwage.org.nz/), [KiwiSaver](https://www.ird.govt.nz/kiwisaver-changes), [holiday pay](https://www.employment.govt.nz/pay-and-hours/pay-and-wages/leave-and-holiday-pay/pay-as-you-go-annual-holiday-payments), [ACC levies](https://acc.co.nz/levy-results)

## Unverified and open risks

- **Which Stripe card rate a Connect marketplace pays:** 2.65% + NZ$0.30 on the standard page, 2.7% + NZ$0.30 on the Connect page. Also whether the 0.25% funds-routing fee stacks with the payout fee (not included). Ask Stripe; piece K02 covers payments.
- **Card fees in other countries** are not researched; only New Zealand is shown.
- **Hetzner** showed every cloud plan as "currently unavailable" on 18 Oct 2026, so the Hetzner setups may not be orderable today.
- **OVHcloud:** which OVH company bills a buyer outside Australia (other catalogues list the same server at US$10 a month), and the IPv4 price in the Auckland Local Zone (control panel only).
- **Server capacity per user** is not load tested for any app.
- **Spare Social:** serving video from R2 under Cloudflare's terms, free PhotoDNA scanning, and self-hosted transcoding speed.
- **Didit's** 500 free checks a month end on 1 Nov 2026; the US$0.33 per check used here stays.
- **Currencies:** many price pages show "$" without naming it; USD is assumed.
- **Apple's** non-profit fee waiver, and whether native iPhone apps ship at all (piece K04).
- **People on-costs:** ACC work levy 0.69% and Working Safer levy 0.08% come from search excerpts.
- **Spare OS:** update size is measured on one example image; downloads per install are assumptions.
- **Incumbents:** Uber's 28% is an illustration; agents' 3 to 4% is from calculator sites; Fiverr's 20% is from secondary sources.
- **Our assumptions:** the example amounts, 10 active users per list-app transaction, a 12-month tenancy, and the team size. The team must grow beyond the pilot, which is not costed.
- **Not included:** legal advice, insurance, licences (such as a passenger service licence), outside accountant and audit, security review before launch, domain names, card disputes and refunds, currency margins on paying offshore suppliers.
