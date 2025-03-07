# DN42 registry_wizard
A collection of tools to interact with DN42 registry data

## Usage
```
Usage: registry_wizard <registry_root> <COMMAND>

Commands:
  roa                    ROA file generation (various formats)
  dns                    DNS zone file and trust anchor generation (for use with PowerDNS)
  object_metadata        Object metadata output (JSON format)
  graph                  Object output with forward and backlinks, path between objects, related objects (JSON / graphviz dot format)
  hierarchical_prefixes  Hierarchical prefix tree output (JSON format)
  explorer               Start web-based registry explorer (including a ROA file server)
  rtr                    Start RTR server for ROA data
  remove                 Safely remove a list of registry objects along with all their dependencies
  mrt_activity           Output active ASNs from MRT RIB dumps along with their last seen time
  help                   Print this message or the help of the given subcommand(s)

Arguments:
  <registry_root>  path to registry root

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### Build notes
For the default build target, ``musl-gcc`` is required. (``musl`` package on Arch Linux)
