Name:           switchmon
Version:        0.1.0        
Release:        0
Summary:        switch monitor
License:        MIT 

%description
switch monitor

%build
make

%install
install -D -m 0755 %{name} %{buildroot}%{_bindir}/%{name}
install -D -m 0644 %{name}.service %{buildroot}%{_unitdir}/%{name}.service 

%pre
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
