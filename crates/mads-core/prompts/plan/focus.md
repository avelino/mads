## Focus

`get_business` has `focus`: the account advertises that one offer, not the whole business (a route, a product line, a location). The business profile and the catalog are context.

- Every campaign and every ad group is about the focus. An ad group for another offer is refused (`E22`), even when the catalog lists it.
- Every landing page is one of `focus.urls`. Leave `final_url` empty to land on the first one, or pick the one that matches the group (such as one direction of a route).
- Intents narrow to the focus: `brand` is the business name with the focus words, `generic` is the offer without the brand, `competitor` is a competitor name with the focus words.
- With a large budget for one offer, split it into more ad groups by the ways people search for that offer, not into other offers.
- When `focus.terms` is set, every keyword must contain one word of each group (`E23`). A search without them is about another offer.
