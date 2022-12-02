complete -c cyme -s d -l vidpid -d 'Show only devices with the specified vendor and product ID numbers (in hexadecimal) in format VID:[PID]' -r
complete -c cyme -s s -l show -d 'Show only devices with specified device and/or bus numbers (in decimal) in format [[bus]:][devnum]' -r
complete -c cyme -s D -l device -d 'Selects which device lsusb will examine - supplied as Linux /dev/bus/usb/BBB/DDD style path' -r
complete -c cyme -l filter-name -d 'Filter on string contained in name' -r
complete -c cyme -l filter-serial -d 'Filter on string contained in serial' -r
complete -c cyme -s b -l blocks -d 'Specify the blocks which will be displayed for each device and in what order' -r -f -a "{bus-number	Number of bus device is attached,device-number	Bus issued device number,branch-position	Position of device in parent branch,port-path	Linux style port path,sys-path	Linux udev reported syspath,driver	Linux udev reported driver loaded for device,icon	Icon based on VID/PID,vendor-id	Unique vendor identifier - purchased from USB IF,product-id	Vendor unique product identifier,name	The device name as reported in descriptor or using usb_ids if None,manufacturer	The device manufacturer as provided in descriptor or using usb_ids if None,product-name	The device product name as reported by usb_ids vidpid lookup,vendor-name	The device vendor name as reported by usb_ids vid lookup,serial	Device serial string as reported by descriptor,speed	Advertised device capable speed,tree-positions	Position along all branches back to trunk device,bus-power	macOS system_profiler only - actually bus current in mA not power!,bus-power-used	macOS system_profiler only - actually bus current used in mA not power!,extra-current-used	macOS system_profiler only - actually bus current used in mA not power!,bcd-device	The device version,bcd-usb	The supported USB version,class-code	Class of interface provided by USB IF - only available when using libusb,sub-class	Sub-class of interface provided by USB IF - only available when using libusb,protocol	Prototol code for interface provided by USB IF - only available when using libusb}"
complete -c cyme -l bus-blocks -d 'Specify the blocks which will be displayed for each bus and in what order' -r -f -a "{bus-number	System bus number identifier,icon	Icon based on VID/PID,name	Bus name from descriptor or usb_ids,host-controller	Host Controller on macOS\, vendor put here when using libusb,pci-vendor	Understood to be vendor ID - it is when using libusb,pci-device	Understood to be product ID - it is when using libusb,pci-revision	Revsision of hardware,port-path	syspath style port path to bus\, applicable to Linux only}"
complete -c cyme -l config-blocks -d 'Specify the blocks which will be displayed for each configuration and in what order' -r -f -a "{name	Name from string descriptor,number	Number of config\, bConfigurationValue; value to set to enable to configuration,num-interfaces	Interfaces available for this configuruation,attributes	Attributes of configuration\, bmAttributes,max-power	Maximum current consumption in mA}"
complete -c cyme -l interface-blocks -d 'Specify the blocks which will be displayed for each interface and in what order' -r -f -a "{name	Name from string descriptor,number	Interface number,port-path	Interface port path\, applicable to Linux,class-code	Class of interface provided by USB IF,sub-class	Sub-class of interface provided by USB IF,protocol	Prototol code for interface provided by USB IF,alt-setting	Interfaces can have the same number but an alternate settings defined here,driver	Driver obtained from udev on Linux only,sys-path	syspath obtained from udev on Linux only,num-endpoints	An interface can have many endpoints,icon	Icon based on ClassCode/SubCode/Protocol}"
complete -c cyme -l endpoint-blocks -d 'Specify the blocks which will be displayed for each endpoint and in what order' -r -f -a "{number	Endpoint number on interface,direction	Direction of data into endpoint,transfer-type	Type of data transfer endpoint accepts,sync-type	Synchronisation type (Iso mode),usage-type	Usage type (Iso mode),max-packet-size	Maximum packet size in bytes endpoint can send/recieve,interval	Interval for polling endpoint data transfers. Value in frame counts. Ignored for Bulk & Control Endpoints. Isochronous must equal 1 and field may range from 1 to 255 for interrupt endpoints}"
complete -c cyme -l sort-devices -d 'Sort devices by value' -r -f -a "{branch-position	Sort by position in parent branch,device-number	Sort by bus device number,no-sort	No sorting; whatever order it was parsed}"
complete -c cyme -l group-devices -d 'Group devices by value when listing' -r -f -a "{no-group	No grouping,bus	Group into buses with bus info as heading - like a flat tree}"
complete -c cyme -s l -l lsusb -d 'Attempt to maintain compatibility with lsusb output'
complete -c cyme -s t -l tree -d 'Dump USB device hierarchy as a tree'
complete -c cyme -s v -l verbose -d 'Verbosity level: 1 prints device configurations; 2 prints interfaces; 3 prints interface endpoints; 4 prints everything and all blocks'
complete -c cyme -l sort-buses -d 'Sort devices by bus number'
complete -c cyme -l hide-buses -d 'Hide empty buses; those with no devices'
complete -c cyme -l hide-hubs -d 'Hide empty hubs; those with no devices'
complete -c cyme -l decimal -d 'Show base16 values as base10 decimal instead'
complete -c cyme -l no-padding -d 'Disable padding to align blocks'
complete -c cyme -l no-colour -d 'Disable coloured output, can also use NO_COLOR environment variable'
complete -c cyme -l headings -d 'Show block headings'
complete -c cyme -l json -d 'Output as json format after sorting, filters and tree settings are applied'
complete -c cyme -l force-libusb -d 'Force libusb mode on macOS rather than using system_profiler output'
complete -c cyme -s c -l debug -d 'Turn debugging information on. Alternatively can use RUST_LOG env: INFO, DEBUG, TRACE'
complete -c cyme -l gen -d 'Generate cli completions and man page'
complete -c cyme -s h -l help -d 'Print help information (use `--help` for more detail)'
complete -c cyme -s V -l version -d 'Print version information'