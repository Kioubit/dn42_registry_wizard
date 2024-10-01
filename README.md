# DN42 registry_wizard
A collection of tools to interact with DN42 registry data

## Usage
```
Usage: registry_wizard <registry_root> <COMMAND>

Commands:
  roa                    ROA file generation (various formats)
  dns                    DNS zone file and trust anchor generation (for use with PowerDNS)
  object_metadata        Object metadata output (JSON format)
  graph                  Registry object output with forward and backlinks (JSON format)
  hierarchical_prefixes  Hierarchical prefix tree output (JSON format)
  remove_mnt             Remove a list of maintainers along with all their objects from the registry
  mrt_activity           Output last seen time for active ASNs in MRT RIB dumps. List inactive maintainers.
  help                   Print this message or the help of the given subcommand(s)

Arguments:
  <registry_root>  path to registry root

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### Build notes
For the default build target, ``musl-gcc`` is required. (``musl`` package on Arch Linux)
