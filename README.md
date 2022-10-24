# Soroban Offices Smart Contract

### Historical inspiration
The paulette, was a law enforced by France in the beginnings of the 17th century. It allowed the king to sell the ownership of public offices (making them hereditary). The owners of these public offices had to pay a tax each $\Delta t$ to keep a bought public office. These offices expire every \(\Delta t\) and the the owner of each office has to pay a tax to maintain the ownership, if they didn't pay this tax, the office could be revoked by the king.

## Actual implementation Abstract
The contract admin can create new offices and put them for sale by invoking the [Soroban Dutch Auction Contract](https://github.com/Xycloo/soroban-dutch-auction-contract). This means that these offices start with a certain price which decreases with time. Users can then buy these offices at the auction's price and become the owners. The offices expire after a week ($604800s$), and can be renewed by paying a tax (ideally less than the price at which the office was bought). If more than a week goes by and the owner hasn't paid the tax yet, the admin can revoke the office and put it for sale in an auction again. 

