## Debby

A simple Rust program to install `.deb` files on non-Debian systems.

With this utility you're able to:
- **Install `.deb` packages**

    Easily install any `.deb` package system-wide by just doing:
    ```sh
    debby install /path/to/deb
    ```
    or using the `i` alias.
- **See all system-wide installed `.deb` packages** 

    Quickly see what `.deb` packages are installed on your system with debby by just doing:
    ```sh
    debby all
    ```
    or use the `a` alias.
- **Uninstall `.deb` packages**

    Easily uninstall any `.deb` package installed with debby by just doing:
    ```sh
    debby uninstall /path/to/installed/deb
    ```
    or using the `u` alias.
    You can also specify a numeric id gotten from the aforementioned subcommand or package name rather than the .deb package.
- **Check whether a particular `.deb` package is installed or not**

    Quickly determine if a specific `.deb` package is installed on your system by just doing:
    ```sh
    debby check /path/to/deb
    ```
    or using the `c` alias.
- **View the contents of `.deb` packages**

    Quickly view the contents of any `.deb` package without installing it on your system by just doing:
    ```sh
    debby view /path/to/deb
    ```
    or using the `v` alias.

### Technical notes
- When you install a `.deb` package, debby keeps track of all the files it adds in a database. This allows it to later uninstall the package cleanly without removing any critical system files
- The database is stored in /root/.local/share/debby/db.sqlite

Tested on *arch btw* but should work on any distro.
