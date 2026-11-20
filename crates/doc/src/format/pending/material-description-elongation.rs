//! A physical material the design defines (`user_materials`) may have a `description` (text, at
//! most 2000 characters) and an `elongation_percent` (elongation at break, above 0 and at most
//! 1000). Both are left out when the material has none, so older designs read and save as before.
//! They sit on the material, not in its engineering card, because a card is read strictly. The
//! last release warns about the newer format number, opens the design with its materials, leaves
//! the two fields out while it has the design open, and writes them back when it saves.
