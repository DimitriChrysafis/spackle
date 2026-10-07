# vcard

vCard 3.0 reader/writer. Handles folded continuation lines per RFC 2425,
typed params (`EMAIL;TYPE=HOME`), multi-card streams, and 75-octet line
folding on output.

```py
cards = parse_text(open("contacts.vcf").read())
cards[0].get("FN")
```
