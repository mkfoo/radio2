Name:           hls_client
Version:        0.2.0
Release:        0
Summary:        HLS client
License:        MIT 

%description
HLS client

%build
cross build --target aarch64-unknown-linux-gnu --release

%install
install -D -m 0755 target/aarch64-unknown-linux-gnu/release/%{name} %{buildroot}%{_bindir}/%{name}
install -D -m 0644 %{name}.service %{buildroot}%{_unitdir}/%{name}.service 
install -D -m 0644 radio.json %{buildroot}%{_sysconfdir}/radio.json

%pre
getent group radio || groupadd -r radio
getent passwd radio || useradd -r -g radio -s /bin/false radio
usermod -a -G audio,dqtt radio
%service_add_pre %{name}.service

%preun
%service_del_preun %{name}.service

%post
%service_add_post %{name}.service

%postun
%service_del_postun %{name}.service

%files
%{_bindir}/%{name}
%{_unitdir}/%{name}.service
%{_sysconfdir}/radio.json
