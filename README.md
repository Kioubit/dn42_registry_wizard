# DN42 registry_wizard
A collection of tools to interact with DN42 registry data

## Usage
```
Usage: registry_wizard <registry_root> <COMMAND>

Commands:
  roa                   ROA file generation (various formats)
  dns                   DNS zone file and trust anchor generation (for use with PowerDNS)
  inetnumMetadata       Inetnum metadata output (JSON format)
  objectMetadata        Object metadata output (JSON format)
  hierarchicalPrefixes  Hierarchical prefix tree output (JSON format)
  mrt_activity          Output last seen time for active ASNs in MRT RIB dumps. List registry resources that are unused.
  help                  Print this message or the help of the given subcommand(s)

Arguments:
  <registry_root>  path to registry root

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### Build notes
For the release target, ``musl-gcc`` is required. (``musl`` package on Arch Linux)
