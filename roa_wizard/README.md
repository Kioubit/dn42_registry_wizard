# roa_wizard

```
Usage: <path to registry root> <action> <flag>

Where <action>:
'v4'   : bird2 v4 format
'v6'   : bird2 v6 format
'json' : json format

Where <flag>:
''       : No flag
'strict' : Abort program if an error was found in a file
```

## Features
- Generate various ROA formats
- Detect invalid fields in the registry such as invalid IP addresses
- Optional strict mode to abort on errors in registry files
- Evaluate filter.txt / filter6.txt
- Focus on reducing execution time
