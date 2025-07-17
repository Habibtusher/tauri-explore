"use client";
import { invoke } from '@tauri-apps/api/core';
import React, { useEffect, useState } from 'react';

const Page = () => {
    const [printers, setPrinters] = useState([]);

    useEffect(() => {
        const fetchPrinters = async () => {
            try {
                const printers = await invoke('list_printers');
                const printerList = await invoke('List_all_printers');
                console.log('Connected printers list:', printerList);
                setPrinters(printers);
            } catch (error) {
                console.error('Error fetching printers:', error);
                setPrinters([]);
            }
        };
        fetchPrinters();
    }, []);

    return (
        <div>
            <h1>Printers</h1>
            <ul>
                {printers.length > 0 ? (
                    printers.map((printer, index) => (
                        <li key={index}>{printer}</li>
                    ))
                ) : (
                    <li>No printers found</li>
                )}
            </ul>
        </div>
    );
};

export default Page;