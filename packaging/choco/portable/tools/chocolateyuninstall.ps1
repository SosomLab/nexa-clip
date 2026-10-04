$ErrorActionPreference = 'Stop'

# The portable build lives only in the package folder - Chocolatey removes the folder and the shim.
# Files the user created next to the executable (portable data) are left untouched.
Write-Host 'Removing Nexa Clip (Portable) - only the package folder is cleaned up.'
