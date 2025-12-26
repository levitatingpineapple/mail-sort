cargo build --release
systemctl stop mail-sort
cp ./target/release/mail-sort /usr/bin/mail-sort
systemctl start mail-sort
