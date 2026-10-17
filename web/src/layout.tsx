import StorageIcon from '@mui/icons-material/Storage';
import { OpsaLogo } from '@opsa-dev/ui-shared';
import { Application, NavBar } from '@opsa-dev/ui-tscl';
import { type PropsWithChildren } from 'react';


export function Layout(props: PropsWithChildren<{
  activeNavigationEntryId: 'servers' | null;
}>) {
  let whoami = null;
  let avatarUrl = null;

  return (
    <Application
      brand={<OpsaLogo title="Datacenter" />}
      account={
        whoami && (
          <a href="https://home.lietar.net" className="account">
            <img src={avatarUrl} />
            <div className="text">{whoami.name}</div>
          </a>
        )
      }
      navigation={
        <NavBar
          leftEntries={[
            {
              id: 'servers',
              active: props.activeNavigationEntryId === 'servers',
              label: 'Servers',
              icon: StorageIcon,
              target: '/',
            },
          ]} />
      }>
      <div className="Body">
        {props.children}
      </div>
    </Application>
  );
}
