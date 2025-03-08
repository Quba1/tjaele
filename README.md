# Tjaele - Fan control for Nvidia GPUs on Linux with Wayland

This software was created for my personal needs and has not been thoroughly tested. **Use at your own risk!**

The software allows you to control the fan curve for NVIDIA GPUs on Linux Desktops using Wayland (X11 should also work but wasn't tested).

To install this software: (1) compile it with cargo, (2) run the installation script from the `utils` folder. No install commands are provided for now, to require users to have a neccessary knowledge before installing this software which might damage their hardware. **Always review the code before running it!**

After the installation edit config file in `/usr/local/etc/tjaele/config.toml` - **the default fan curve might damage your device**. Then restart `tjaeled` service with `systemctl`.

Run `tjaele` command to check if everything works.
