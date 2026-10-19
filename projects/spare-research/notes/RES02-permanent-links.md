# RES02: Permanent links for citing

*Piece RES02, Spare Research. Written 2026-10-20. An estimate for planning, not a quote. Money in US dollars, excluding tax.*

## The question

Spare Research papers are free to read and free to publish. Every paper needs a link that never breaks and that people can cite, and each version needs a clear way to be cited too. Which system should give the links, who runs it, what does it cost, and who pays?

## Options

| Option | Run by | Fees (US$) | What it needs from us | Versions | Fit for a free platform |
| --- | --- | --- | --- | --- | --- |
| Crossref DOI | Crossref, a DOI registration agency | Membership US$200 (revenue and expenses under US$1,000) or US$275 (under US$1 million) a year. Preprint-type record US$0.25 now, US$0.20 from Jan 2027; journal article US$1.00 now, US$0.95 from 2027 | Accurate metadata at publication, a DOI that always resolves, DOIs in reference citations, current URLs | New DOI per stage; accepted and published versions share one; a significant update gets a new DOI | Good: tiny fixed cost, small per-paper cost |
| DataCite DOI | DataCite, a DOI registration agency | EUR 2,000 a year plus EUR 200 to 700 shared infrastructure; no per-DOI fee below 100,001 a year (USD unverified) | Six required properties, a public landing page, tombstones for removed items | New DOI for a major version; optional grouping DOI | Fine, but a high fixed cost at low volume |
| ARK | ARK Alliance (keeps the NAAN registry) | Free to create (provider charges unverified) | A NAAN, our own persistence policy and resolver | Suffix marks variants | Cheapest; we carry the whole promise alone |
| Handle | CNRI registry, DONA Foundation | US$50 once plus US$50 a year per prefix | A prefix and our own handle server | None built in | The layer under DOIs, with no citation rules of its own |
| SWHID | Software Heritage (ISO/IEC 18670:2025) | No fee stated (unverified) | Code saved in the archive; ID computed from content | One ID per commit, release or file | For code only |
| ORCID | ORCID, a non-profit | Free for people; organisation membership from US$1,490 a year | Identifies authors, not papers | Not applicable | Use for authors; membership not needed now |

## Evidence

- Crossref fees, the 2027 changes, terms and duties (accurate metadata, hyperlinked DOIs, DOIs never deleted, free metadata edits): [fees](https://www.crossref.org/fees/), [2027 update](https://www.crossref.org/blog/fee-update-lowering-and-removing-doi-registration-fees-in-january-2027/), [terms](https://www.crossref.org/membership/terms/), [maintaining records](https://www.crossref.org/documentation/register-maintain-records/maintaining-your-metadata/), [references](https://www.crossref.org/documentation/principles-practices/best-practices/references/), [preprint markup](https://www.crossref.org/documentation/schema-library/markup-guide-record-types/posted-content-includes-preprints/), [versioning](https://www.crossref.org/documentation/principles-practices/best-practices/versioning/).
- DataCite: [fees](https://datacite.org/fees/), [versioning](https://support.datacite.org/docs/versioning), [landing pages](https://support.datacite.org/docs/landing-pages), [schema](https://datacite-metadata-schema.readthedocs.io/en/4.6/properties/overview/).
- How others version: arXiv gives one DOI per paper pointing to the latest version ([arXiv](https://info.arxiv.org/help/doi.html)); bioRxiv and medRxiv one DOI for all versions, cited with a version number ([openRxiv](https://connect.biorxiv.org/news/2025/11/18/preprint_dois)); Zenodo a DOI per version plus an all-versions DOI ([Zenodo](https://support.zenodo.org/help/en-gb/1-upload-deposit/97-what-is-doi-versioning)); OSF a DOI per update through Crossref ([OSF](https://www.cos.io/blog/doi-versioning-and-metaror)).
- ARK: [about](https://arks.org/about/), [getting started](https://arks.org/about/getting-started-implementing-arks/). Handle: [fees](https://www.handle.net/payment.html), [DOI key facts](https://www.doi.org/the-identifier/resources/factsheets/key-facts-on-digital-object-identifier-system). SWHID: [swhid.org](https://www.swhid.org/). ORCID: [membership](https://info.orcid.org/membership), [Public API](https://info.orcid.org/documentation/features/public-api/).
- Our arithmetic: DataCite's fixed cost is about US$2,500 to US$3,000 at the rate in [K06](../../kit/notes/K06-cost-model.md). Crossref at US$275 plus US$0.20 a paper stays cheaper until about 11,000 to 14,000 papers a year; at 5,000 papers a year it is about US$1,275.

## Recommendation

1. **DOI agency: Crossref, as a direct member.** It has a preprint record type, links to the published article, and the lowest cost at our size.
2. **One DOI per work, not per version,** with an opaque suffix and no version number, resolving to the latest version (as arXiv and bioRxiv do).
3. **Every version gets its own permanent address** on our site ending in v2, v3 and so on, never edited. A withdrawn version stays as a tombstone page. The version number and a change summary go in the Crossref record.
4. **A new DOI only for a significant rewrite** that changes how the work is read or credited (Crossref's own rule). Corrections are separate notices with their own DOI.
5. **Who pays: Spare Research, as a running cost on the open cost ledger.** Authors, readers and reviewers pay nothing. Budget US$275 a year plus US$0.20 a paper.
6. **Authors: ORCID iDs** collected and put in each record; sign-in through the free Public API; no membership.
7. **Code: Software Heritage.** Authors archive their code and cite the SWHID.
8. **ARK and Handle: not used.** ARK is the backup if DOI costs ever become unaffordable.

## Open risks

- Crossref may expect reviewed final papers to use the journal article type (US$0.95, not US$0.20). Ask before the first deposit.
- The promise is forever: DOIs cannot be deleted and must always resolve. Crossref wants a third-party archive ([RES05](RES05-long-term-preservation.md)). A closure plan is needed before launch.
- Exact-version citing rests on our version address, not the DOI. If users want a DOI per version, revisit (OSF does it on Crossref).
- Fees change; the cut to US$0.20 is announced, not yet in force.
- Crossref membership is an agreement, so a legal entity must sign it ([K01](../../kit/notes/K01-legal-entity.md)).
- Unverified: DataCite fees in US dollars, ARK provider costs, Software Heritage fees, the ORCID Public API price, and whether the Handle fee page is current.

## Sources

Every source is linked in Evidence above. Researched 20 October 2026.
