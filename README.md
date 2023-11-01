# StrongBox® Contract
​
The purpose of this smart contract is to facilitate secure creation, management, and end-to-end encrypted access of all StrongBox® account(s) hosted on Secret Network.
​
## Functions
​
- Instantiate
​
This function allows any user to instantiate a StrongBox® contract by providing at least 32 bytes of initial seed data. A user may choose to instantiate an empty StrongBox® used to store caller address(es) for the user/owner.
​
- Update StrongBox®
​
This function strictly controls access to a user's StrongBox® and the contents stored within. Only the owner of a StrongBox® account will be granted access to update the contents of a StrongBox®. Any/all unauthorized attempts to access a StrongBox® account will be met with an "unauthorized error."
​
- Create Viewing Key
​
This function allows a StrongBox® account owner to create a viewing key assigned to the address of their choice, as long as at least 20 bytes of entropy are provided along with the chosen address. Once both conditions are met, the viewing key will be securely delivered to the owner's chosen address.
​
- Query StrongBox®
​
This function strictly limits the ability to view the contents within a StrongBox® account to the account owner in possession of the viewing key.

- Transfer StrongBox® Ownership
​
This function allows a StrongBox® owner to securely transfer ownership of a StrongBox® account to a new owner. Once the transfer of StrongBox® account ownership is complete, only the new owner will have admin rights over that specific StrongBox® account.
​
- Revoke StrongBox® Viewing Key
​
This function allows a StrongBox® owner to revoke a viewing key associated with a specific viewer. Once the revoke viewing key is complete, viewer can't query strongbox with the old viewing key.
