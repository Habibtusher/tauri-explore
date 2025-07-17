"use client";
import { invoke } from '@tauri-apps/api/core';
import React, { useEffect, useState } from 'react';

const DevicesPage = () => {
    const [devices, setDevices] = useState([]);
    const [error, setError] = useState(null);

    useEffect(() => {
        const fetchDevices = async () => {
            // if (!window.__TAURI__) {
            //     setError('Tauri API not available. Ensure the app is running with `yarn tauri dev`.');
            //     return;
            // }
            try {
                const devices = await invoke('list_devices');
                console.log('Connected devices:', devices);
                setDevices(devices);
                setError(null);
            } catch (error) {
                console.error('Error fetching devices:', error);
                setError(error.message || 'Failed to fetch devices');
                setDevices([]);
            }
        };
        fetchDevices();
    }, []);

    return (
        <div>
            <h1>Connected Keyboards and Mice</h1>
            {error && <p style={{ color: 'red' }}>{error}</p>}
            <ul>
                {devices.length > 0 ? (
                    devices.map((device, index) => (
                        <li key={index}>{device}</li>
                    ))
                ) : (
                    <li>No devices found</li>
                )}
            </ul>
        </div>
    );
};

export default DevicesPage;